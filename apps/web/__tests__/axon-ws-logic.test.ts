import { describe, expect, it } from 'vitest'

/**
 * Tests for the pure logic patterns used in use-axon-ws.ts.
 * The hook itself is React-dependent, but the constants, backoff math,
 * and message queue capping logic are testable as pure functions.
 */

const BASE_BACKOFF = 1000
const MAX_BACKOFF = 30000
const MAX_PENDING_MESSAGES = 100

function computeBackoff(attempts: number): number {
  return Math.min(BASE_BACKOFF * 2 ** attempts, MAX_BACKOFF)
}

function capPendingMessages<T>(messages: T[]): T[] {
  if (messages.length > MAX_PENDING_MESSAGES) {
    return messages.slice(-MAX_PENDING_MESSAGES)
  }
  return messages
}

describe('WebSocket backoff computation', () => {
  it('starts at BASE_BACKOFF (1s) for attempt 0', () => {
    expect(computeBackoff(0)).toBe(1000)
  })

  it('doubles each attempt', () => {
    expect(computeBackoff(1)).toBe(2000)
    expect(computeBackoff(2)).toBe(4000)
    expect(computeBackoff(3)).toBe(8000)
  })

  it('caps at MAX_BACKOFF (30s)', () => {
    expect(computeBackoff(10)).toBe(MAX_BACKOFF)
    expect(computeBackoff(20)).toBe(MAX_BACKOFF)
    expect(computeBackoff(100)).toBe(MAX_BACKOFF)
  })

  it('reaches MAX_BACKOFF between attempt 4 and 5', () => {
    // 2^4 * 1000 = 16000 < 30000
    expect(computeBackoff(4)).toBe(16000)
    // 2^5 * 1000 = 32000 > 30000 → capped
    expect(computeBackoff(5)).toBe(MAX_BACKOFF)
  })
})

describe('pending message queue capping', () => {
  it('returns array unchanged when under limit', () => {
    const msgs = Array.from({ length: 50 }, (_, i) => ({ id: i }))
    expect(capPendingMessages(msgs)).toHaveLength(50)
  })

  it('returns array unchanged when exactly at limit', () => {
    const msgs = Array.from({ length: MAX_PENDING_MESSAGES }, (_, i) => ({ id: i }))
    expect(capPendingMessages(msgs)).toHaveLength(MAX_PENDING_MESSAGES)
  })

  it('trims to last MAX_PENDING_MESSAGES when over limit', () => {
    const msgs = Array.from({ length: MAX_PENDING_MESSAGES + 20 }, (_, i) => ({ id: i }))
    const result = capPendingMessages(msgs)
    expect(result).toHaveLength(MAX_PENDING_MESSAGES)
    // Should keep the most recent messages (last N)
    expect(result[0]).toEqual({ id: 20 })
    expect(result[result.length - 1]).toEqual({ id: MAX_PENDING_MESSAGES + 19 })
  })

  it('handles empty array', () => {
    expect(capPendingMessages([])).toEqual([])
  })
})

describe('constants', () => {
  it('BASE_BACKOFF is 1 second', () => {
    expect(BASE_BACKOFF).toBe(1000)
  })

  it('MAX_BACKOFF is 30 seconds', () => {
    expect(MAX_BACKOFF).toBe(30000)
  })

  it('MAX_PENDING_MESSAGES is 100', () => {
    expect(MAX_PENDING_MESSAGES).toBe(100)
  })
})
