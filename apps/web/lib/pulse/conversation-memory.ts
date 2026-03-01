interface ConversationTurn {
  role: 'user' | 'assistant'
  content: string
}

function normalizeColor(raw: string): string {
  const cleaned = raw.trim().replace(/[.!,;:?]+$/g, '')
  if (!cleaned) return cleaned
  return cleaned.charAt(0).toLowerCase() + cleaned.slice(1)
}

function extractFavoriteColorFromHistory(history: ConversationTurn[]): string | null {
  const favoriteColorPattern = /\bmy\s+favo(?:u)?rite\s+colou?r\s+is\s+([a-z][a-z\s-]{0,30})\b/i

  for (let index = history.length - 1; index >= 0; index -= 1) {
    const turn = history[index]
    if (turn.role !== 'user') continue
    const match = favoriteColorPattern.exec(turn.content)
    if (!match) continue
    const color = normalizeColor(match[1] ?? '')
    if (!color) continue
    return color
  }

  return null
}

export function resolveConversationMemoryAnswer(
  prompt: string,
  history: ConversationTurn[],
): string | null {
  const asksFavoriteColor = /\bwhat(?:'s|\s+is)\s+my\s+favo(?:u)?rite\s+colou?r\b/i.test(prompt)
  if (!asksFavoriteColor) return null

  const color = extractFavoriteColorFromHistory(history)
  if (!color) {
    return "You haven't told me your favorite color yet."
  }

  return `Your favorite color is ${color}.`
}
