import { describe, expect, it } from 'vitest'
import { formatStructuredText, summarizeStructuredValue } from '@/lib/structured-text'

describe('formatStructuredText', () => {
  it('formats null as "none"', () => {
    expect(formatStructuredText(null)).toBe('none')
  })

  it('formats strings as-is', () => {
    expect(formatStructuredText('hello world')).toBe('hello world')
  })

  it('formats numbers', () => {
    expect(formatStructuredText(42)).toBe('42')
    expect(formatStructuredText(0)).toBe('0')
    expect(formatStructuredText(-3.14)).toBe('-3.14')
  })

  it('formats NaN/Infinity as "not-a-number"', () => {
    expect(formatStructuredText(Number.NaN)).toBe('not-a-number')
    expect(formatStructuredText(Number.POSITIVE_INFINITY)).toBe('not-a-number')
  })

  it('formats booleans as yes/no', () => {
    expect(formatStructuredText(true)).toBe('yes')
    expect(formatStructuredText(false)).toBe('no')
  })

  it('formats empty array as "(empty list)"', () => {
    expect(formatStructuredText([])).toBe('(empty list)')
  })

  it('formats scalar array as bullet list', () => {
    expect(formatStructuredText(['a', 'b', 'c'])).toBe('- a\n- b\n- c')
  })

  it('formats nested array items with "Item N" labels', () => {
    const result = formatStructuredText([{ x: 1 }])
    expect(result).toContain('- Item 1')
    expect(result).toContain('x: 1')
  })

  it('formats empty object as "(empty object)"', () => {
    expect(formatStructuredText({})).toBe('(empty object)')
  })

  it('formats object with undefined values as "(empty object)"', () => {
    expect(formatStructuredText({ a: undefined })).toBe('(empty object)')
  })

  it('formats flat object as key-value lines', () => {
    const result = formatStructuredText({ name: 'test', count: 3 })
    expect(result).toBe('name: test\ncount: 3')
  })

  it('humanizes snake_case keys', () => {
    const result = formatStructuredText({ some_key: 'val' })
    expect(result).toContain('some key: val')
  })

  it('humanizes camelCase keys', () => {
    const result = formatStructuredText({ someKey: 'val' })
    expect(result).toContain('some Key: val')
  })

  it('indents nested objects', () => {
    const result = formatStructuredText({ outer: { inner: 'deep' } })
    expect(result).toContain('outer:')
    expect(result).toContain('  inner: deep')
  })

  it('handles deeply nested structures', () => {
    const result = formatStructuredText({ a: { b: { c: 'leaf' } } })
    expect(result).toContain('a:')
    expect(result).toContain('  b:')
    expect(result).toContain('    c: leaf')
  })

  it('falls back to String() for unknown types', () => {
    const sym = Symbol('test')
    expect(formatStructuredText(sym)).toBe('Symbol(test)')
  })
})

describe('summarizeStructuredValue', () => {
  it('truncates long strings to 96 chars', () => {
    const long = 'x'.repeat(200)
    const result = summarizeStructuredValue(long)
    expect(result).toHaveLength(96)
    expect(result).toMatch(/\.\.\.$/u)
  })

  it('keeps short strings intact', () => {
    expect(summarizeStructuredValue('short')).toBe('short')
  })

  it('summarizes scalars', () => {
    expect(summarizeStructuredValue(null)).toBe('none')
    expect(summarizeStructuredValue(true)).toBe('yes')
    expect(summarizeStructuredValue(42)).toBe('42')
  })

  it('summarizes empty array', () => {
    expect(summarizeStructuredValue([])).toBe('(empty list)')
  })

  it('previews first 3 array items', () => {
    expect(summarizeStructuredValue([1, 2, 3])).toBe('1, 2, 3')
  })

  it('adds ellipsis for arrays > 3 items', () => {
    expect(summarizeStructuredValue([1, 2, 3, 4])).toBe('1, 2, 3, ...')
  })

  it('labels nested arrays in preview', () => {
    expect(summarizeStructuredValue([[1, 2]])).toBe('list(2)')
  })

  it('labels objects in array as "object"', () => {
    expect(summarizeStructuredValue([{ a: 1 }])).toBe('object')
  })

  it('summarizes object with first 3 keys', () => {
    const result = summarizeStructuredValue({ a: 1, b: 'hi', c: true })
    expect(result).toBe('a: 1 | b: hi | c: yes')
  })

  it('adds ellipsis for objects > 3 keys', () => {
    const result = summarizeStructuredValue({ a: 1, b: 2, c: 3, d: 4 })
    expect(result).toMatch(/\| \.\.\.$/u)
  })

  it('labels nested arrays in object summary', () => {
    const result = summarizeStructuredValue({ items: [1, 2, 3] })
    expect(result).toBe('items: list(3)')
  })

  it('labels nested objects in object summary', () => {
    const result = summarizeStructuredValue({ nested: { x: 1 } })
    expect(result).toBe('nested: object')
  })
})
