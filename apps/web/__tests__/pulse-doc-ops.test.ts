import { describe, expect, it } from 'vitest'
import { isHighRiskOperationSet, validateDocOperations } from '@/lib/pulse/doc-ops'
import type { DocOperation } from '@/lib/pulse/types'

describe('doc-ops validator', () => {
  it('accepts a single small append', () => {
    const ops: DocOperation[] = [{ type: 'append_markdown', markdown: 'Short text.' }]
    const result = validateDocOperations(ops, '# Existing doc content here')
    expect(result.valid).toBe(true)
    expect(result.reasons).toHaveLength(0)
  })

  it('flags replace_document changing >40% of chars', () => {
    const original = 'A'.repeat(1000)
    const ops: DocOperation[] = [{ type: 'replace_document', markdown: 'B'.repeat(1000) }]
    expect(isHighRiskOperationSet(ops, original)).toBe(true)
  })

  it('flags single insert >1200 chars', () => {
    const ops: DocOperation[] = [{ type: 'append_markdown', markdown: 'X'.repeat(1201) }]
    expect(isHighRiskOperationSet(ops, '')).toBe(true)
  })

  it('flags >3 operations in one set', () => {
    const ops: DocOperation[] = [
      { type: 'append_markdown', markdown: 'a' },
      { type: 'append_markdown', markdown: 'b' },
      { type: 'append_markdown', markdown: 'c' },
      { type: 'append_markdown', markdown: 'd' },
    ]
    expect(isHighRiskOperationSet(ops, '')).toBe(true)
  })

  it('flags ops that remove a heading', () => {
    const original = '# Title\n\nContent\n\n## Section\n\nMore'
    const ops: DocOperation[] = [
      { type: 'replace_document', markdown: '# Title\n\nContent\n\nMore' },
    ]
    const result = validateDocOperations(ops, original)
    expect(result.reasons).toContain('removes_heading')
  })

  it('small safe ops are not high risk', () => {
    const ops: DocOperation[] = [{ type: 'append_markdown', markdown: 'Just a short note.' }]
    expect(isHighRiskOperationSet(ops, '# Existing doc')).toBe(false)
  })
})
