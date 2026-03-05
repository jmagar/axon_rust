import { createHash } from 'node:crypto'
import type { PulseChatStreamEvent } from '@/lib/pulse/chat-stream'

type ReplayCacheEntry = {
  events: PulseChatStreamEvent[]
  sizeBytes: number
  updatedAt: number
}

export const REPLAY_BUFFER_LIMIT = 512
export const REPLAY_CACHE_TTL_MS = 2 * 60_000
export const REPLAY_CACHE_MAX_ENTRIES = 64
export const REPLAY_CACHE_MAX_TOTAL_BYTES = 8 * 1024 * 1024

// Module-level singleton — intentional. All concurrent requests share this
// cache so that client reconnects can replay missed events from any request.
export const replayCache = new Map<string, ReplayCacheEntry>()
let runningTotalBytes = 0

function estimateEventBytes(event: PulseChatStreamEvent): number {
  try {
    return Buffer.byteLength(JSON.stringify(event), 'utf8')
  } catch {
    return 0
  }
}

function estimateBufferBytes(events: PulseChatStreamEvent[]): number {
  return events.reduce((total, event) => total + estimateEventBytes(event), 0)
}

/**
 * Evict oldest entries when the cache exceeds MAX_ENTRIES or MAX_TOTAL_BYTES.
 * Map iteration order is insertion order, so the first keys are the oldest.
 */
export function evictOldestEntries(): void {
  while (
    replayCache.size > REPLAY_CACHE_MAX_ENTRIES ||
    runningTotalBytes > REPLAY_CACHE_MAX_TOTAL_BYTES
  ) {
    const oldest = replayCache.keys().next()
    if (oldest.done) break
    const entry = replayCache.get(oldest.value)
    if (entry) runningTotalBytes -= entry.sizeBytes
    replayCache.delete(oldest.value)
  }
}

export function pruneReplayCache(now: number): void {
  for (const [key, entry] of replayCache.entries()) {
    if (now - entry.updatedAt > REPLAY_CACHE_TTL_MS) {
      runningTotalBytes -= entry.sizeBytes
      replayCache.delete(key)
    }
  }
}

export function upsertReplayEntry(
  key: string,
  events: PulseChatStreamEvent[],
  now = Date.now(),
): void {
  const existing = replayCache.get(key)
  if (existing) {
    runningTotalBytes -= existing.sizeBytes
    // Delete and re-insert to refresh Map insertion order (most recently used)
    replayCache.delete(key)
  }
  const sizeBytes = estimateBufferBytes(events)
  runningTotalBytes += sizeBytes
  replayCache.set(key, { events, sizeBytes, updatedAt: now })
  evictOldestEntries()
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
  agent: string
  model: string
}): string {
  return createHash('sha256').update(JSON.stringify(data)).digest('hex')
}
