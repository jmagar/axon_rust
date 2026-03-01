export interface ParsedMessage {
  role: 'user' | 'assistant'
  content: string
}

/** Maximum byte length of a single JSONL line we are willing to parse. */
const MAX_LINE_BYTES = 512_000 // 512 KB

/**
 * Parse Claude Code JSONL session content into structured messages.
 * Port of the Rust logic in crates/ingest/sessions/claude.rs.
 * Pure function — no I/O.
 */
export function parseClaudeJsonl(raw: string): ParsedMessage[] {
  const messages: ParsedMessage[] = []

  // Strip null bytes before any further processing.
  const sanitized = raw.replace(/\0/g, '')
  for (const line of sanitized.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed) continue

    // Reject lines that exceed the per-line size cap.
    // Buffer.byteLength counts UTF-8 bytes, not UTF-16 code units, so multi-byte
    // characters are correctly accounted for (trimmed.length would undercount them).
    if (Buffer.byteLength(trimmed, 'utf8') > MAX_LINE_BYTES) continue

    let val: Record<string, unknown>
    try {
      val = JSON.parse(trimmed) as Record<string, unknown>
    } catch {
      continue
    }

    const type = val.type
    if (type !== 'user' && type !== 'assistant') continue
    const role = type as 'user' | 'assistant'

    const msg = val.message as Record<string, unknown> | undefined
    const msgContent = msg?.content

    let text = ''
    if (typeof msgContent === 'string') {
      text = msgContent
    } else if (Array.isArray(msgContent)) {
      for (const block of msgContent) {
        const blockText = (block as Record<string, unknown>).text
        if (typeof blockText === 'string') text += `${blockText}\n`
      }
    } else {
      continue
    }

    if (text.trim()) {
      messages.push({ role, content: text.trim() })
    }
  }

  return messages
}
