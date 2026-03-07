import { randomUUID } from 'node:crypto'
import { NextResponse } from 'next/server'
import { z } from 'zod'
import { ensureRepoRootEnvLoaded } from '@/lib/pulse/server-env'
import type { SavedDocMeta } from '@/lib/pulse/storage'
import { savePulseDoc, updatePulseDoc } from '@/lib/pulse/storage'

/**
 * Rewrite Docker-internal hostnames to localhost with mapped ports when
 * running outside Docker (local dev). Mirrors the Rust CLI's
 * `normalize_local_service_url()` logic.
 */
const DOCKER_HOST_MAP: Record<string, string> = {
  'axon-qdrant:6333': '127.0.0.1:53333',
  'axon-qdrant:6334': '127.0.0.1:53334',
}

function resolveLocalUrl(raw: string | undefined): string | undefined {
  if (!raw) return raw
  try {
    const url = new URL(raw)
    const hostPort = `${url.hostname}:${url.port || (url.protocol === 'https:' ? '443' : '80')}`
    const mapped = DOCKER_HOST_MAP[hostPort]
    if (mapped) {
      const [host, port] = mapped.split(':')
      url.hostname = host
      url.port = port
      return url.toString().replace(/\/$/, '')
    }
  } catch {
    /* malformed URL — return as-is */
  }
  return raw
}

const SaveRequestSchema = z.object({
  title: z.string().min(1).max(200),
  markdown: z.string().max(200_000),
  tags: z.array(z.string()).optional(),
  collections: z.array(z.string()).optional(),
  embed: z.boolean().default(true),
  /** Must be a valid slugified filename produced by savePulseDoc. */
  filename: z
    .string()
    .min(1)
    .max(255)
    .regex(/^[a-z0-9-]+-\d+\.md$/, 'Invalid pulse filename format')
    .optional(),
  /** Client-cached from last save response — passed back to skip file read on updates. */
  createdAt: z.string().optional(),
  /** Client-cached updatedAt — triggers server-side concurrent-edit detection. */
  updatedAt: z.string().optional(),
})

/** GET first; only PUT on 404 — safe to call on existing collections. */
async function ensureCollection(
  qdrantUrl: string,
  collection: string,
  vectorSize: number,
): Promise<void> {
  const getRes = await fetch(`${qdrantUrl}/collections/${encodeURIComponent(collection)}`)
  if (getRes.ok) return
  if (getRes.status !== 404) {
    throw new Error(`Qdrant collection check failed: ${getRes.status}`)
  }
  const createRes = await fetch(`${qdrantUrl}/collections/${encodeURIComponent(collection)}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ vectors: { size: vectorSize, distance: 'Cosine' } }),
  })
  if (!createRes.ok) {
    throw new Error(
      `Qdrant collection create failed: ${createRes.status} ${await createRes.text().catch(() => '')}`,
    )
  }
}

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

    const {
      title,
      markdown,
      tags,
      collections,
      embed,
      filename: incomingFilename,
      createdAt: incomingCreatedAt,
      updatedAt: incomingUpdatedAt,
    } = parsed.data
    const meta: SavedDocMeta = incomingFilename
      ? await updatePulseDoc(incomingFilename, {
          title,
          markdown,
          tags,
          collections,
          createdAt: incomingCreatedAt,
          clientUpdatedAt: incomingUpdatedAt,
        })
      : await savePulseDoc({ title, markdown, tags, collections })
    const { filename } = meta

    if (embed) {
      const teiUrl = process.env.TEI_URL
      const qdrantUrl = resolveLocalUrl(process.env.QDRANT_URL)
      const collection = collections?.[0] ?? process.env.AXON_COLLECTION ?? 'cortex'

      if (teiUrl && qdrantUrl && markdown.trim()) {
        try {
          // Pre-delete existing vectors before re-embedding to prevent accumulation.
          // ?wait=true ensures delete is applied before the upsert begins.
          if (incomingFilename) {
            await fetch(
              `${qdrantUrl}/collections/${encodeURIComponent(collection)}/points/delete?wait=true`,
              {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                  filter: { must: [{ key: 'url', match: { value: `pulse://${filename}` } }] },
                }),
              },
            ).catch((err) => console.error('[Pulse] Pre-delete failed (continuing):', err))
          }

          const chunks = chunkText(markdown, 2000, 200)
          const embedResponse = await fetch(`${teiUrl}/embed`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ inputs: chunks }),
          })

          if (!embedResponse.ok) {
            const body = await embedResponse.text().catch(() => '')
            console.error('[Pulse] TEI embed failed:', embedResponse.status, body)
          } else {
            const vectors = (await embedResponse.json()) as number[][]
            const vectorSize = vectors[0]?.length
            if (!vectorSize) {
              throw new Error('[Pulse] Embed response returned no vectors')
            }
            await ensureCollection(qdrantUrl, collection, vectorSize)
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

    return NextResponse.json({
      filename,
      saved: true,
      createdAt: meta.createdAt,
      updatedAt: meta.updatedAt,
      tags: meta.tags,
      collections: meta.collections,
    })
  } catch (err) {
    console.error('[Pulse] Save route error:', err)
    return NextResponse.json({ error: 'Save failed' }, { status: 500 })
  }
}
