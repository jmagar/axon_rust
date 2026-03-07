/**
 * Converts raw WS command.output.json items to a markdown string for the editor.
 * Used when a Cmd+K command completes in the background and opens a new editor tab.
 */

import type { AskResult, QueryResult } from '@/lib/result-types'

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

export function resultToMarkdown(mode: string, items: unknown[]): string {
  if (items.length === 0) return ''
  const first = items[0]

  if (mode === 'ask' && isRecord(first) && typeof first.answer === 'string') {
    const r = first as unknown as AskResult
    const heading = typeof r.query === 'string' && r.query ? `# ${r.query}\n\n` : ''
    return `${heading}${r.answer}`
  }

  if (mode === 'research') {
    if (isRecord(first)) {
      const body = first.report ?? first.answer ?? first.content ?? first.output
      if (typeof body === 'string') {
        const query = typeof first.query === 'string' ? first.query : ''
        return query ? `# ${query}\n\n${body}` : body
      }
    }
    if (typeof first === 'string') return first
  }

  if (mode === 'query') {
    const isQueryList = items.every(
      (item): item is QueryResult =>
        isRecord(item) && typeof item.url === 'string' && typeof item.snippet === 'string',
    )
    if (isQueryList) {
      return (items as QueryResult[])
        .map((item, i) => `## ${i + 1}. [${item.url}](${item.url})\n\n${item.snippet}`)
        .join('\n\n---\n\n')
    }
  }

  if (mode === 'retrieve' && isRecord(first) && typeof first.content === 'string') {
    const url = typeof first.url === 'string' ? first.url : ''
    return url ? `# ${url}\n\n${first.content as string}` : (first.content as string)
  }

  // Fallback: JSON code block
  const payload = items.length === 1 ? items[0] : items
  return `\`\`\`json\n${JSON.stringify(payload, null, 2)}\n\`\`\``
}
