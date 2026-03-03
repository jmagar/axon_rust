import { describe, expect, it } from 'vitest'

describe('useTimedNotice', () => {
  it('exports a function', async () => {
    const { useTimedNotice } = await import('@/hooks/use-timed-notice')
    expect(typeof useTimedNotice).toBe('function')
  })

  it('module can be imported without errors', async () => {
    const mod = await import('@/hooks/use-timed-notice')
    expect(mod).toHaveProperty('useTimedNotice')
  })
})
