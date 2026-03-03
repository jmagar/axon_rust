import { describe, expect, it } from 'vitest'
import { fmtMs } from '@/components/results/shared'

describe('fmtMs', () => {
  it('formats sub-second as ms', () => {
    expect(fmtMs(42)).toBe('42ms')
  })

  it('formats 0 as ms', () => {
    expect(fmtMs(0)).toBe('0ms')
  })

  it('formats 999 as ms', () => {
    expect(fmtMs(999)).toBe('999ms')
  })

  it('formats exactly 1000 as seconds', () => {
    expect(fmtMs(1000)).toBe('1.0s')
  })

  it('formats 1500 as seconds', () => {
    expect(fmtMs(1500)).toBe('1.5s')
  })

  it('formats large values as seconds', () => {
    expect(fmtMs(12345)).toBe('12.3s')
  })

  it('rounds to one decimal place', () => {
    expect(fmtMs(1999)).toBe('2.0s')
  })
})
