import { describe, expect, it } from 'vitest'
import { checkPermission } from '@/lib/pulse/permissions'
import type { DocOperation } from '@/lib/pulse/types'

describe('pulse permissions', () => {
  it('plan mode: blocks edit operations', () => {
    const ops: DocOperation[] = [{ type: 'append_markdown', markdown: 'note' }]
    const result = checkPermission('plan', ops, { isCurrentDoc: true })
    expect(result.allowed).toBe(false)
  })

  it('accept-edits mode: blocks ops on other documents', () => {
    const ops: DocOperation[] = [{ type: 'append_markdown', markdown: 'note' }]
    const result = checkPermission('accept-edits', ops, { isCurrentDoc: false })
    expect(result.allowed).toBe(false)
  })

  it('accept-edits mode: requires confirmation for edits', () => {
    const ops: DocOperation[] = [{ type: 'replace_document', markdown: 'X'.repeat(2000) }]
    const result = checkPermission('accept-edits', ops, {
      isCurrentDoc: true,
      currentDocMarkdown: 'A'.repeat(100),
    })
    expect(result.allowed).toBe(true)
    expect(result.requiresConfirmation).toBe(true)
  })

  it('bypass-permissions mode: allows everything without confirmation', () => {
    const ops: DocOperation[] = [{ type: 'replace_document', markdown: 'anything' }]
    const result = checkPermission('bypass-permissions', ops, { isCurrentDoc: false })
    expect(result.allowed).toBe(true)
    expect(result.requiresConfirmation).toBe(false)
  })
})
