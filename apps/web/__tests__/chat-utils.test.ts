import { describe, expect, it } from 'vitest'
import {
  computeMessageVirtualWindow,
  formatStreamPhaseLabel,
  MESSAGE_ESTIMATED_HEIGHT,
  MESSAGE_VIRTUAL_THRESHOLD,
} from '@/components/pulse/chat-utils'

describe('computeMessageVirtualWindow', () => {
  it('returns full range when below threshold', () => {
    const result = computeMessageVirtualWindow(50, 0, 800)
    expect(result).toEqual({ shouldVirtualize: false, start: 0, end: 50 })
  })

  it('returns full range when exactly at threshold', () => {
    const result = computeMessageVirtualWindow(MESSAGE_VIRTUAL_THRESHOLD, 0, 800)
    expect(result).toEqual({
      shouldVirtualize: false,
      start: 0,
      end: MESSAGE_VIRTUAL_THRESHOLD,
    })
  })

  it('virtualizes when above threshold', () => {
    const result = computeMessageVirtualWindow(MESSAGE_VIRTUAL_THRESHOLD + 1, 0, 800)
    expect(result.shouldVirtualize).toBe(true)
  })

  it('start is clamped to 0 when scroll is at top', () => {
    const result = computeMessageVirtualWindow(500, 0, 800)
    expect(result.start).toBe(0)
  })

  it('end is clamped to totalMessages', () => {
    const total = 200
    const result = computeMessageVirtualWindow(total, 0, 100000)
    expect(result.end).toBeLessThanOrEqual(total)
  })

  it('computes correct start from scroll offset', () => {
    const scrollOffset = MESSAGE_ESTIMATED_HEIGHT * 50
    const result = computeMessageVirtualWindow(500, scrollOffset, 800)
    // floor(scrollOffset / height) - overscan = 50 - 8 = 42
    expect(result.start).toBe(42)
  })

  it('computes visible slice in the middle', () => {
    const scrollOffset = MESSAGE_ESTIMATED_HEIGHT * 100
    const viewport = MESSAGE_ESTIMATED_HEIGHT * 5
    const result = computeMessageVirtualWindow(500, scrollOffset, viewport)
    // start = floor(100*h / h) - 8 = 92
    // visibleCount = ceil(5*h / h) + 16 = 21
    // end = min(500, 92 + 21) = 113
    expect(result.start).toBe(92)
    expect(result.end).toBe(113)
  })

  it('handles zero viewport by treating as 1px', () => {
    const result = computeMessageVirtualWindow(200, 0, 0)
    expect(result.shouldVirtualize).toBe(true)
    // ceil(1/156) + 16 = 17, clamped start=0
    expect(result.start).toBe(0)
    expect(result.end).toBe(17)
  })

  it('handles 0 totalMessages', () => {
    const result = computeMessageVirtualWindow(0, 0, 800)
    expect(result).toEqual({ shouldVirtualize: false, start: 0, end: 0 })
  })
})

describe('formatStreamPhaseLabel', () => {
  it('returns Starting for started', () => {
    expect(formatStreamPhaseLabel('started')).toBe('Starting')
  })

  it('returns Thinking for thinking', () => {
    expect(formatStreamPhaseLabel('thinking')).toBe('Thinking')
  })

  it('returns Finalizing for finalizing', () => {
    expect(formatStreamPhaseLabel('finalizing')).toBe('Finalizing')
  })

  it('returns Thinking for null', () => {
    expect(formatStreamPhaseLabel(null)).toBe('Thinking')
  })

  it('returns Thinking for undefined', () => {
    expect(formatStreamPhaseLabel(undefined)).toBe('Thinking')
  })
})
