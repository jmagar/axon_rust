import { describe, expect, it } from 'vitest'
import { MODE_CATEGORY_LABELS, MODE_CATEGORY_ORDER, MODES, NO_INPUT_MODES } from '@/lib/ws-protocol'

describe('ws-protocol mode registry', () => {
  it('does not include a workspace category', () => {
    expect(MODE_CATEGORY_ORDER).not.toContain('workspace')
    expect((MODE_CATEGORY_LABELS as Record<string, string>).workspace).toBeUndefined()
  })

  it('does not include pulse mode in MODES', () => {
    const pulse = MODES.find((m) => m.id === 'pulse')
    expect(pulse).toBeUndefined()
    expect(NO_INPUT_MODES.has('pulse')).toBe(false)
  })
})
