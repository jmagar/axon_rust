import { describe, expect, it } from 'vitest'
import { getCommandSpec } from '@/lib/axon-command-map'

describe('axon-command-map: pulse removal', () => {
  it('does not expose pulse as an executable command', () => {
    expect(getCommandSpec('pulse')).toBeUndefined()
  })
})
