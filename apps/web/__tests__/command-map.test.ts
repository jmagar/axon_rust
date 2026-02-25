import { describe, expect, it } from 'vitest'
import { getCommandSpec } from '@/lib/axon-command-map'

describe('axon-command-map: pulse', () => {
  it('has a command spec for pulse', () => {
    const spec = getCommandSpec('pulse')
    expect(spec).toBeDefined()
  })

  it('pulse spec has correct properties', () => {
    const spec = getCommandSpec('pulse')
    expect(spec?.category).toBe('workspace')
    expect(spec?.input).toBe('text')
    expect(spec?.asyncByDefault).toBe(false)
    expect(spec?.supportsJobs).toBe(false)
    expect(spec?.renderIntent).toBe('workspace')
  })
})
