interface ParsedAssistantPayload {
  text: string
  operations: unknown[]
}

function normalizeCodeFences(input: string): string {
  const trimmed = input.trim()
  const fenceMatch = /^```(?:json)?\s*([\s\S]*?)\s*```$/i.exec(trimmed)
  if (!fenceMatch) return input
  return fenceMatch[1] ?? input
}

function findBalancedJsonObject(input: string): string | null {
  const source = input
  let start = -1
  let depth = 0
  let inString = false
  let escaped = false

  for (let i = 0; i < source.length; i += 1) {
    const ch = source[i]
    if (inString) {
      if (escaped) {
        escaped = false
        continue
      }
      if (ch === '\\') {
        escaped = true
        continue
      }
      if (ch === '"') inString = false
      continue
    }

    if (ch === '"') {
      inString = true
      continue
    }
    if (ch === '{') {
      if (depth === 0) start = i
      depth += 1
      continue
    }
    if (ch === '}') {
      if (depth === 0) continue
      depth -= 1
      if (depth === 0 && start >= 0) {
        return source.slice(start, i + 1)
      }
    }
  }

  return null
}

function parseObject(value: unknown): ParsedAssistantPayload | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const record = value as Record<string, unknown>
  const text = typeof record.text === 'string' ? record.text : ''
  const operations = Array.isArray(record.operations) ? record.operations : []
  return { text, operations }
}

export function parseClaudeAssistantPayload(raw: string): ParsedAssistantPayload | null {
  const direct = tryParsePayload(raw)
  if (direct) return direct

  const noFence = normalizeCodeFences(raw)
  const fenced = tryParsePayload(noFence)
  if (fenced) return fenced

  const embedded = findBalancedJsonObject(noFence)
  if (!embedded) return null
  return tryParsePayload(embedded)
}

export function fallbackAssistantText(raw: string): string {
  const trimmed = normalizeCodeFences(raw).trim()
  if (!trimmed) return 'No assistant text returned.'

  const jsonLike =
    trimmed.startsWith('{') ||
    trimmed.startsWith('[') ||
    trimmed.startsWith('```json') ||
    /^"\s*text"\s*:/i.test(trimmed)
  if (jsonLike) {
    return 'Assistant response was structured but unreadable. Please retry.'
  }

  const maybeJson = findBalancedJsonObject(trimmed)
  if (maybeJson && maybeJson === trimmed) {
    return 'Assistant response was structured but unreadable. Please retry.'
  }

  return trimmed
}

function tryParsePayload(raw: string): ParsedAssistantPayload | null {
  try {
    const parsed = JSON.parse(raw)
    return parseObject(parsed)
  } catch {
    return null
  }
}
