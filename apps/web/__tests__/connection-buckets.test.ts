import { describe, expect, it } from 'vitest'
import { buildConnectionBuckets } from '@/components/neural-canvas/synapse'
import type { SynapticConnectionRef } from '@/components/neural-canvas/types'

function mockConn(bucket: 0 | 1 | 2): SynapticConnectionRef {
  return {
    preTerminal: { x: 0, y: 0 },
    dendriteTip: { x: 10, y: 10 },
    strength: 0.5,
    baseAlpha: 0.1,
    bucket,
  }
}

describe('buildConnectionBuckets', () => {
  it('returns empty buckets for empty input', () => {
    const result = buildConnectionBuckets([])
    expect(result).toEqual({ strong: [], medium: [], faint: [] })
  })

  it('partitions connections by bucket value', () => {
    const conns = [mockConn(0), mockConn(2), mockConn(1), mockConn(0), mockConn(2)]
    const result = buildConnectionBuckets(conns)
    expect(result.strong).toHaveLength(2)
    expect(result.medium).toHaveLength(1)
    expect(result.faint).toHaveLength(2)
  })

  it('puts bucket 0 into strong', () => {
    const conn = mockConn(0)
    const result = buildConnectionBuckets([conn])
    expect(result.strong).toEqual([conn])
    expect(result.medium).toEqual([])
    expect(result.faint).toEqual([])
  })

  it('puts bucket 1 into medium', () => {
    const conn = mockConn(1)
    const result = buildConnectionBuckets([conn])
    expect(result.medium).toEqual([conn])
  })

  it('puts bucket 2 into faint', () => {
    const conn = mockConn(2)
    const result = buildConnectionBuckets([conn])
    expect(result.faint).toEqual([conn])
  })

  it('preserves insertion order within each bucket', () => {
    const a = mockConn(0)
    const b = mockConn(0)
    const c = mockConn(0)
    const result = buildConnectionBuckets([a, b, c])
    expect(result.strong[0]).toBe(a)
    expect(result.strong[1]).toBe(b)
    expect(result.strong[2]).toBe(c)
  })
})
