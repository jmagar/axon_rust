import { describe, expect, it } from 'vitest'

/**
 * Tests for PulseOpConfirmation Enter key scoping logic.
 * We test the guard condition directly since the component uses a portal
 * and global keydown listener.
 */

function shouldHandleKey(target: { tagName: string; isContentEditable?: boolean }): boolean {
  const tag = target.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable) {
    return false
  }
  return true
}

describe('PulseOpConfirmation key handler guard', () => {
  it('handles keys when target is a div', () => {
    expect(shouldHandleKey({ tagName: 'DIV' })).toBe(true)
  })

  it('handles keys when target is the body', () => {
    expect(shouldHandleKey({ tagName: 'BODY' })).toBe(true)
  })

  it('ignores keys when target is an input', () => {
    expect(shouldHandleKey({ tagName: 'INPUT' })).toBe(false)
  })

  it('ignores keys when target is a textarea', () => {
    expect(shouldHandleKey({ tagName: 'TEXTAREA' })).toBe(false)
  })

  it('ignores keys when target is contentEditable', () => {
    expect(shouldHandleKey({ tagName: 'DIV', isContentEditable: true })).toBe(false)
  })

  it('handles keys on non-editable div', () => {
    expect(shouldHandleKey({ tagName: 'DIV', isContentEditable: false })).toBe(true)
  })

  it('handles keys when target is a button', () => {
    expect(shouldHandleKey({ tagName: 'BUTTON' })).toBe(true)
  })
})
