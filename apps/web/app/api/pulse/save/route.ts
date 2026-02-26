import { randomUUID } from 'node:crypto'
import { NextResponse } from 'next/server'
import { z } from 'zod'
import { ensureRepoRootEnvLoaded } from '@/lib/pulse/server-env'
import { savePulseDoc } from '@/lib/pulse/storage'

const SaveRequestSchema = z.object({
  title: z.string().min(1).max(200),
  markdown: z.string().max(200_000),
  tags: z.array(z.string()).optional(),
  collections: z.array(z.string()).optional(),
  embed: z.boolean().default(true),
})

function chunkText(text: string, size: number, overlap: number): string[] {
  if (size <= 0 || overlap < 0 || size <= overlap) {
    return [text]
  }
  const chunks: string[] = []
  let start = 0
  while (start < text.length) {
    chunks.push(text.slice(start, start + size))
    start += size - overlap
  }
  return chunks
}

export async function POST(request: Request) {
  try {
    ensureRepoRootEnvLoaded()
    const body = await request.json()
    const parsed = SaveRequestSchema.safeParse(body)
    if (!parsed.success) {
      return NextResponse.json(
        { error: parsed.error.issues[0]?.message ?? 'Invalid request payload' },
        { status: 400 },
      )
    }

    const { title, markdown, tags, collections, embed } = parsed.data
    const { path, filename } = await savePulseDoc({
      title,
      markdown,
      tags,
      collections,
    })

    if (embed) {
      const teiUrl = process.env.TEI_URL
      const qdrantUrl = process.env.QDRANT_URL
      const collection = collections?.[0] ?? 'pulse'

      if (teiUrl && qdrantUrl && markdown.trim()) {
        try {
          const chunks = chunkText(markdown, 2000, 200)
          const embedResponse = await fetch(`${teiUrl}/embed`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ inputs: chunks }),
          })

          if (embedResponse.ok) {
            const vectors = (await embedResponse.json()) as number[][]
            const points = vectors.map((vector, i) => ({
              id: randomUUID(),
              vector,
              payload: {
                text: chunks[i],
                url: `pulse://${filename}`,
                title,
                doc_type: 'pulse_note',
                chunk_index: i,
              },
            }))

            const qdrantRes = await fetch(
              `${qdrantUrl}/collections/${encodeURIComponent(collection)}/points?wait=true`,
              {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ points }),
              },
            )
            if (!qdrantRes.ok) {
              console.error(
                '[Pulse] Qdrant upsert failed:',
                collection,
                filename,
                qdrantRes.status,
                await qdrantRes.text().catch(() => ''),
              )
            }
          }
        } catch (err) {
          console.error('[Pulse] Embed failed (save succeeded):', err)
        }
      }
    }

    return NextResponse.json({ path, filename, saved: true })
  } catch (err) {
    console.error('[Pulse] Save route error:', err)
    return NextResponse.json({ error: 'Save failed' }, { status: 500 })
  }
}
