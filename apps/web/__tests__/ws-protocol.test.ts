import { describe, expect, it } from 'vitest'
import {
  isWorkspaceMode,
  MODE_CATEGORY_LABELS,
  MODE_CATEGORY_ORDER,
  MODES,
  NO_INPUT_MODES,
} from '@/lib/ws-protocol'

describe('ws-protocol mode registry', () => {
  it('includes workspace in ModeCategory', () => {
    expect(MODE_CATEGORY_ORDER).toContain('workspace')
  })

  it('has a label for workspace category', () => {
    expect(MODE_CATEGORY_LABELS.workspace).toBe('Workspace')
  })

  it('places workspace after service in category order', () => {
    const idx = MODE_CATEGORY_ORDER.indexOf('workspace')
    const serviceIdx = MODE_CATEGORY_ORDER.indexOf('service')
    expect(idx).toBeGreaterThan(serviceIdx)
  })

  it('includes pulse mode in MODES', () => {
    const pulse = MODES.find((m) => m.id === 'pulse')
    expect(pulse).toBeDefined()
    expect(pulse?.category).toBe('workspace')
    expect(pulse?.label).toBe('Pulse')
  })

  it('pulse is NOT in NO_INPUT_MODES', () => {
    expect(NO_INPUT_MODES.has('pulse')).toBe(false)
  })

  it('pulse is a workspace mode', () => {
    expect(isWorkspaceMode('pulse')).toBe(true)
  })

  it('scrape is NOT a workspace mode', () => {
    expect(isWorkspaceMode('scrape')).toBe(false)
  })
})
