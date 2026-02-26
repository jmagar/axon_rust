const URL_PATTERN = /\bhttps?:\/\/[^\s<>"')\]]+/gi

export type PulsePromptIntent =
  | { kind: 'source'; urls: string[] }
  | { kind: 'chat'; prompt: string }

function dedupeUrls(urls: string[]): string[] {
  const seen = new Set<string>()
  const unique: string[] = []
  for (const raw of urls) {
    const normalized = raw.trim().replace(/[.,;:!?]+$/, '')
    if (!normalized || seen.has(normalized)) continue
    seen.add(normalized)
    unique.push(normalized)
  }
  return unique
}

function isLikelySourceCommand(promptLower: string): boolean {
  return (
    promptLower.startsWith('+source ') ||
    promptLower.startsWith('source ') ||
    promptLower.startsWith('add source') ||
    promptLower.startsWith('crawl ') ||
    promptLower.startsWith('scrape ') ||
    promptLower.startsWith('index ')
  )
}

export function detectPulsePromptIntent(prompt: string): PulsePromptIntent {
  const trimmed = prompt.trim()
  if (!trimmed) return { kind: 'chat', prompt: '' }

  const urls = dedupeUrls(trimmed.match(URL_PATTERN) ?? [])
  if (urls.length === 0) {
    return { kind: 'chat', prompt: trimmed }
  }

  const lower = trimmed.toLowerCase()
  const pureUrlInput = urls.length === 1 && trimmed === urls[0]
  if (pureUrlInput || isLikelySourceCommand(lower)) {
    return { kind: 'source', urls }
  }

  return { kind: 'chat', prompt: trimmed }
}
