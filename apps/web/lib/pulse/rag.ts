import type { PulseChatRequest, PulseCitation } from './types'

interface QdrantPoint {
  payload?: {
    text?: string
    url?: string
    title?: string
    collection?: string
  }
  score?: number
}

interface QdrantSearchResponse {
  result?: QdrantPoint[]
}

function truncate(input: string, max = 300): string {
  if (input.length <= max) return input
  return `${input.slice(0, max)}...`
}

function excerptDocument(markdown: string, maxChars = 4000): string {
  return truncate(markdown, maxChars)
}

export async function retrieveFromCollections(
  query: string,
  selectedCollections: string[],
  limit = 4,
): Promise<PulseCitation[]> {
  const qdrantUrl = process.env.QDRANT_URL
  const teiUrl = process.env.TEI_URL
  if (!qdrantUrl || !teiUrl) return []

  try {
    const embedRes = await fetch(`${teiUrl}/embed`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ inputs: [query] }),
    })
    if (!embedRes.ok) return []

    const embedJson = (await embedRes.json()) as unknown
    const queryVector = Array.isArray(embedJson)
      ? (embedJson[0] as number[] | undefined)
      : undefined
    if (!queryVector) return []

    const perCollection = await Promise.all(
      selectedCollections.map(async (collection) => {
        const response = await fetch(`${qdrantUrl}/collections/${collection}/points/search`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            vector: queryVector,
            limit,
            with_payload: true,
          }),
        })

        if (!response.ok) return [] as PulseCitation[]
        const data = (await response.json()) as QdrantSearchResponse
        return (data.result ?? []).map((point) => {
          const payload = point.payload ?? {}
          return {
            url: payload.url ?? '',
            title: payload.title ?? 'Untitled',
            snippet: truncate(payload.text ?? '', 280),
            collection,
            score: point.score ?? 0,
          } satisfies PulseCitation
        })
      }),
    )

    return perCollection
      .flat()
      .sort((a, b) => b.score - a.score)
      .slice(0, limit * 2)
  } catch {
    return []
  }
}

export function buildPulseSystemPrompt(req: PulseChatRequest, citations: PulseCitation[]): string {
  const doc = excerptDocument(req.documentMarkdown)
  const citationContext = citations
    .map((c, i) => `(${i + 1}) [${c.collection}] ${c.title} ${c.url}\n${c.snippet}`)
    .join('\n\n')

  return [
    'You are Pulse, a document copilot for editing markdown safely.',
    'Return plain assistant text and optional doc operations.',
    'Only suggest operations with these types: replace_document, append_markdown, insert_section.',
    `Permission level: ${req.permissionLevel}.`,
    'Current document:',
    doc,
    'Retrieved context:',
    citationContext || 'No citations retrieved.',
  ].join('\n\n')
}
