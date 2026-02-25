import type { ModeDefinition } from '@/lib/ws-protocol'

export type OmniboxPhase =
  | 'idle'
  | 'mode-mention'
  | 'mode-selected'
  | 'file-mention'
  | 'ready'
  | 'executing'

export type MentionKind = 'mode' | 'file' | 'none'

export interface LocalDocFile {
  id: string
  label: string
  path: string
  source: 'docs' | 'pulse'
  updatedAt?: string
}

export interface ActiveMentionToken {
  query: string
  start: number
  end: number
}

export function extractActiveMention(input: string): ActiveMentionToken | null {
  const match = /(?:^|\s)@([A-Za-z0-9._/-]*)$/.exec(input)
  if (!match) return null

  const mention = match[1] ?? ''
  const end = input.length
  const start = end - mention.length - 1
  return { query: mention, start, end }
}

export function getMentionKind(input: string, token: ActiveMentionToken | null): MentionKind {
  if (!token) return 'none'
  const trimmed = input.trim()
  const isPureMention = token.start === 0 && trimmed.startsWith('@') && !trimmed.includes(' ')
  return isPureMention ? 'mode' : 'file'
}

export function rankModeSuggestions(
  modes: readonly ModeDefinition[],
  query: string,
  limit = 3,
): ModeDefinition[] {
  const q = query.trim().toLowerCase()
  if (!q) return []

  const startsWith = modes.filter(
    (m) => m.id.toLowerCase().startsWith(q) || m.label.toLowerCase().startsWith(q),
  )
  const contains = modes.filter(
    (m) =>
      !startsWith.some((s) => s.id === m.id) &&
      (m.id.toLowerCase().includes(q) || m.label.toLowerCase().includes(q)),
  )

  return [...startsWith, ...contains].slice(0, limit)
}

function subsequenceScore(candidate: string, query: string): number {
  let qi = 0
  let score = 0
  for (let i = 0; i < candidate.length && qi < query.length; i += 1) {
    if (candidate[i] === query[qi]) {
      score += 8
      if (
        i === 0 ||
        candidate[i - 1] === '/' ||
        candidate[i - 1] === '-' ||
        candidate[i - 1] === '_'
      ) {
        score += 2
      }
      qi += 1
    } else {
      score -= 1
    }
  }
  return qi === query.length ? score : Number.NEGATIVE_INFINITY
}

export function rankFileSuggestions(
  files: LocalDocFile[],
  query: string,
  recentSelections: Record<string, number>,
  limit = 3,
): LocalDocFile[] {
  const q = query.trim().toLowerCase()
  if (!q) return []

  const now = Date.now()

  const ranked = files
    .map((file) => {
      const label = file.label.toLowerCase()
      const path = file.path.toLowerCase()

      let score = Number.NEGATIVE_INFINITY

      if (label === q) score = 1200
      else if (label.startsWith(q)) score = 1000 - (label.length - q.length)
      else if (label.includes(q)) score = 800 - label.indexOf(q) * 2
      else if (path.startsWith(q)) score = 650
      else if (path.includes(q)) score = 500 - path.indexOf(q)
      else score = subsequenceScore(label, q)

      if (!Number.isFinite(score)) return { file, score }

      const recentAt = recentSelections[file.id]
      if (recentAt) {
        const ageMs = Math.max(1, now - recentAt)
        score += Math.max(0, 120 - ageMs / 10_000)
      }

      return { file, score }
    })
    .filter((entry) => Number.isFinite(entry.score))
    .sort((a, b) => b.score - a.score)

  return ranked.slice(0, limit).map((entry) => entry.file)
}

export function replaceActiveMention(
  input: string,
  token: ActiveMentionToken,
  replacement: string,
): string {
  const prefix = input.slice(0, token.start)
  const suffix = input.slice(token.end)
  return `${prefix}${replacement}${suffix}`
}

export function extractMentionLabels(input: string): string[] {
  const labels = new Set<string>()
  const regex = /@([A-Za-z0-9._/-]+)/g
  for (const match of input.matchAll(regex)) {
    const label = match[1]
    if (label) labels.add(label)
  }
  return [...labels]
}

export function deriveOmniboxPhase(params: {
  isProcessing: boolean
  input: string
  mentionKind: MentionKind
  hasModeFeedback: boolean
}): OmniboxPhase {
  const { isProcessing, input, mentionKind, hasModeFeedback } = params
  if (isProcessing) return 'executing'
  if (mentionKind === 'mode') return 'mode-mention'
  if (mentionKind === 'file') return 'file-mention'
  if (hasModeFeedback) return 'mode-selected'
  if (!input.trim()) return 'idle'
  return 'ready'
}
