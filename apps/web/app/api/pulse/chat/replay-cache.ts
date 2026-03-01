import { createHash } from 'node:crypto'
import type { PulseChatStreamEvent } from '@/lib/pulse/chat-stream'

type ReplayCacheEntry = {
  events: PulseChatStreamEvent[]
  updatedAt: number
}

export const REPLAY_BUFFER_LIMIT = 512
export const REPLAY_CACHE_TTL_MS = 2 * 60_000

// Module-level singleton — intentional. All concurrent requests share this
// cache so that client reconnects can replay missed events from any request.
export const replayCache = new Map<string, ReplayCacheEntry>()

export function pruneReplayCache(now: number): void {
  for (const [key, entry] of replayCache.entries()) {
    if (now - entry.updatedAt > REPLAY_CACHE_TTL_MS) {
      replayCache.delete(key)
    }
  }
}

// Periodic TTL eviction — runs once per module load on the server side only.
if (typeof window === 'undefined') {
  setInterval(() => pruneReplayCache(Date.now()), 60_000)
}

export function computeReplayKey(data: {
  prompt: string
  documentMarkdown: string
  selectedCollections: string[]
  threadSources: string[]
  scrapedContext: unknown
  conversationHistory: unknown[]
  permissionLevel: string
  model: string
}): string {
  return createHash('sha256').update(JSON.stringify(data)).digest('hex')
}
