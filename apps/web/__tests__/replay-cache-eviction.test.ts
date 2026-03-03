import { afterEach, describe, expect, it } from 'vitest'
import {
  evictOldestEntries,
  REPLAY_CACHE_MAX_ENTRIES,
  REPLAY_CACHE_TTL_MS,
  replayCache,
} from '@/app/api/pulse/chat/replay-cache'

describe('REPLAY_CACHE_MAX_ENTRIES constant', () => {
  it('is 64', () => {
    expect(REPLAY_CACHE_MAX_ENTRIES).toBe(64)
  })
})

describe('evictOldestEntries', () => {
  afterEach(() => {
    replayCache.clear()
  })

  it('does nothing when cache is under limit', () => {
    for (let i = 0; i < 10; i++) {
      replayCache.set(`key-${i}`, { events: [], sizeBytes: 0, updatedAt: Date.now() })
    }
    evictOldestEntries()
    expect(replayCache.size).toBe(10)
  })

  it('does nothing when cache is exactly at limit', () => {
    for (let i = 0; i < REPLAY_CACHE_MAX_ENTRIES; i++) {
      replayCache.set(`key-${i}`, { events: [], sizeBytes: 0, updatedAt: Date.now() })
    }
    evictOldestEntries()
    expect(replayCache.size).toBe(REPLAY_CACHE_MAX_ENTRIES)
  })

  it('evicts oldest entries when cache exceeds limit', () => {
    for (let i = 0; i < REPLAY_CACHE_MAX_ENTRIES + 5; i++) {
      replayCache.set(`key-${i}`, { events: [], sizeBytes: 0, updatedAt: Date.now() + i })
    }
    evictOldestEntries()
    expect(replayCache.size).toBe(REPLAY_CACHE_MAX_ENTRIES)
    // First 5 entries should be removed (oldest by insertion order)
    expect(replayCache.has('key-0')).toBe(false)
    expect(replayCache.has('key-1')).toBe(false)
    expect(replayCache.has('key-2')).toBe(false)
    expect(replayCache.has('key-3')).toBe(false)
    expect(replayCache.has('key-4')).toBe(false)
    // Last entry should be kept
    expect(replayCache.has(`key-${REPLAY_CACHE_MAX_ENTRIES + 4}`)).toBe(true)
  })

  it('handles empty cache', () => {
    expect(() => evictOldestEntries()).not.toThrow()
    expect(replayCache.size).toBe(0)
  })

  it('evicts down to exactly MAX_ENTRIES', () => {
    for (let i = 0; i < REPLAY_CACHE_MAX_ENTRIES * 2; i++) {
      replayCache.set(`key-${i}`, { events: [], sizeBytes: 0, updatedAt: Date.now() })
    }
    evictOldestEntries()
    expect(replayCache.size).toBe(REPLAY_CACHE_MAX_ENTRIES)
  })

  it('preserves newer entries while evicting older ones', () => {
    // Add entries in order — Map preserves insertion order
    for (let i = 0; i < REPLAY_CACHE_MAX_ENTRIES + 3; i++) {
      replayCache.set(`entry-${i}`, { events: [], sizeBytes: 0, updatedAt: i })
    }
    evictOldestEntries()
    // The first 3 (oldest by insertion) should be gone
    expect(replayCache.has('entry-0')).toBe(false)
    expect(replayCache.has('entry-1')).toBe(false)
    expect(replayCache.has('entry-2')).toBe(false)
    // entry-3 should still exist (it became the oldest remaining)
    expect(replayCache.has('entry-3')).toBe(true)
  })

  it('works correctly with TTL pruning combined', () => {
    const now = Date.now()
    // Fill cache with a mix of stale and fresh entries beyond the limit
    for (let i = 0; i < REPLAY_CACHE_MAX_ENTRIES + 10; i++) {
      const updatedAt = i < 5 ? now - REPLAY_CACHE_TTL_MS - 1 : now
      replayCache.set(`mixed-${i}`, { events: [], sizeBytes: 0, updatedAt })
    }
    // Eviction only cares about count, not TTL
    evictOldestEntries()
    expect(replayCache.size).toBe(REPLAY_CACHE_MAX_ENTRIES)
  })
})
