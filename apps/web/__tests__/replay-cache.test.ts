import { afterEach, describe, expect, it } from 'vitest'
import {
  computeReplayKey,
  pruneReplayCache,
  REPLAY_BUFFER_LIMIT,
  REPLAY_CACHE_TTL_MS,
  replayCache,
} from '@/app/api/pulse/chat/replay-cache'

function makeReplayInput(overrides: Partial<Parameters<typeof computeReplayKey>[0]> = {}) {
  return {
    prompt: 'test prompt',
    documentMarkdown: '',
    selectedCollections: ['cortex'],
    threadSources: [],
    scrapedContext: null,
    conversationHistory: [],
    permissionLevel: 'accept-edits',
    agent: 'claude',
    model: 'sonnet',
    ...overrides,
  }
}

describe('computeReplayKey', () => {
  it('returns a hex string', () => {
    const key = computeReplayKey(makeReplayInput())
    expect(key).toMatch(/^[0-9a-f]{64}$/u)
  })

  it('produces deterministic output', () => {
    const a = computeReplayKey(makeReplayInput())
    const b = computeReplayKey(makeReplayInput())
    expect(a).toBe(b)
  })

  it('produces different keys for different prompts', () => {
    const a = computeReplayKey(makeReplayInput({ prompt: 'hello' }))
    const b = computeReplayKey(makeReplayInput({ prompt: 'world' }))
    expect(a).not.toBe(b)
  })

  it('produces different keys for different models', () => {
    const a = computeReplayKey(makeReplayInput({ model: 'sonnet' }))
    const b = computeReplayKey(makeReplayInput({ model: 'opus' }))
    expect(a).not.toBe(b)
  })

  it('produces different keys for different collections', () => {
    const a = computeReplayKey(makeReplayInput({ selectedCollections: ['cortex'] }))
    const b = computeReplayKey(makeReplayInput({ selectedCollections: ['other'] }))
    expect(a).not.toBe(b)
  })
})

describe('pruneReplayCache', () => {
  afterEach(() => {
    replayCache.clear()
  })

  it('removes entries older than TTL', () => {
    const now = Date.now()
    replayCache.set('old', { events: [], sizeBytes: 0, updatedAt: now - REPLAY_CACHE_TTL_MS - 1 })
    replayCache.set('fresh', { events: [], sizeBytes: 0, updatedAt: now })

    pruneReplayCache(now)

    expect(replayCache.has('old')).toBe(false)
    expect(replayCache.has('fresh')).toBe(true)
  })

  it('keeps entries exactly at TTL boundary', () => {
    const now = Date.now()
    replayCache.set('boundary', { events: [], sizeBytes: 0, updatedAt: now - REPLAY_CACHE_TTL_MS })

    pruneReplayCache(now)

    expect(replayCache.has('boundary')).toBe(true)
  })

  it('handles empty cache', () => {
    expect(() => pruneReplayCache(Date.now())).not.toThrow()
  })

  it('removes all stale entries at once', () => {
    const now = Date.now()
    for (let i = 0; i < 10; i++) {
      replayCache.set(`stale-${i}`, {
        events: [],
        sizeBytes: 0,
        updatedAt: now - REPLAY_CACHE_TTL_MS - 1000,
      })
    }
    replayCache.set('keeper', { events: [], sizeBytes: 0, updatedAt: now })

    pruneReplayCache(now)

    expect(replayCache.size).toBe(1)
    expect(replayCache.has('keeper')).toBe(true)
  })
})

describe('constants', () => {
  it('REPLAY_BUFFER_LIMIT is 512', () => {
    expect(REPLAY_BUFFER_LIMIT).toBe(512)
  })

  it('REPLAY_CACHE_TTL_MS is 2 minutes', () => {
    expect(REPLAY_CACHE_TTL_MS).toBe(120_000)
  })
})
