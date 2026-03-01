// ── Chat pane pure helpers and constants ──────────────────────────────────────
// Zero React imports. All values are stable primitives or pure functions.

export const CHAT_SCROLL_STORAGE_KEY = 'axon.web.pulse.chat-scroll'
export const SOURCE_LIST_SCROLL_STORAGE_KEY = 'axon.web.pulse.source-list-scroll'
export const SOURCE_EXPANDED_STORAGE_KEY = 'axon.web.pulse.sources-expanded'
export const SOURCE_LIST_OPEN_STORAGE_KEY = 'axon.web.pulse.sources-open'

export const MESSAGE_VIRTUAL_THRESHOLD = 120
export const MESSAGE_ESTIMATED_HEIGHT = 156
export const MESSAGE_OVERSCAN = 8

export const SOURCE_VIRTUAL_THRESHOLD = 40
export const SOURCE_ROW_HEIGHT = 24
export const SOURCE_OVERSCAN = 6

/**
 * Compute the slice window for virtualised message rendering.
 *
 * When the conversation is short (below MESSAGE_VIRTUAL_THRESHOLD) the full
 * list is returned unchanged.  For long conversations only the messages that
 * fall within the visible viewport – plus an overscan buffer above and below –
 * are rendered, reducing DOM node count dramatically.
 */
export function computeMessageVirtualWindow(
  totalMessages: number,
  scrollOffset: number,
  viewportPx: number,
): { shouldVirtualize: boolean; start: number; end: number } {
  const shouldVirtualize = totalMessages > MESSAGE_VIRTUAL_THRESHOLD
  if (!shouldVirtualize) {
    return { shouldVirtualize: false, start: 0, end: totalMessages }
  }
  const start = Math.max(0, Math.floor(scrollOffset / MESSAGE_ESTIMATED_HEIGHT) - MESSAGE_OVERSCAN)
  const visibleCount =
    Math.ceil(Math.max(viewportPx, 1) / MESSAGE_ESTIMATED_HEIGHT) + MESSAGE_OVERSCAN * 2
  const end = Math.min(totalMessages, start + visibleCount)
  return { shouldVirtualize: true, start, end }
}

/**
 * Format a UNIX-ms timestamp as a short locale time string (HH:MM).
 * Returns an empty string when the timestamp is absent.
 */
export function formatMessageTime(createdAt: number | undefined): string {
  if (!createdAt) return ''
  return new Date(createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

/**
 * Convert a stream phase value into a human-readable label shown in the
 * "Claude thinking…" loading indicator.
 */
export function formatStreamPhaseLabel(
  phase: 'started' | 'thinking' | 'finalizing' | null | undefined,
): string {
  if (phase === 'started') return 'Starting'
  if (phase === 'thinking') return 'Thinking'
  if (phase === 'finalizing') return 'Finalizing'
  return 'Thinking'
}
