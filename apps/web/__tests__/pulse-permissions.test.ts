import { describe, expect, it } from 'vitest'
import { checkPermission } from '@/lib/pulse/permissions'
import type { DocOperation } from '@/lib/pulse/types'

describe('pulse permissions', () => {
  it('plan mode: allows ops on current document', () => {
    const ops: DocOperation[] = [{ type: 'append_markdown', markdown: 'note' }]
    const result = checkPermission('plan', ops, { isCurrentDoc: true })
    expect(result.allowed).toBe(true)
  })

  it('plan mode: blocks ops on other documents', () => {
    const ops: DocOperation[] = [{ type: 'append_markdown', markdown: 'note' }]
    const result = checkPermission('plan', ops, { isCurrentDoc: false })
    expect(result.allowed).toBe(false)
  })

  it('training-wheels mode: requires confirmation for high-risk ops', () => {
    const ops: DocOperation[] = [{ type: 'replace_document', markdown: 'X'.repeat(2000) }]
    const result = checkPermission('training-wheels', ops, {
      isCurrentDoc: true,
      currentDocMarkdown: 'A'.repeat(100),
    })
    expect(result.allowed).toBe(true)
    expect(result.requiresConfirmation).toBe(true)
  })

  it('full-access mode: allows everything without confirmation', () => {
    const ops: DocOperation[] = [{ type: 'replace_document', markdown: 'anything' }]
    const result = checkPermission('full-access', ops, { isCurrentDoc: false })
    expect(result.allowed).toBe(true)
    expect(result.requiresConfirmation).toBe(false)
  })
})
