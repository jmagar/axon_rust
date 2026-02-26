import { describe, expect, it } from 'vitest'
import { fallbackAssistantText, parseClaudeAssistantPayload } from '@/lib/pulse/claude-response'

describe('pulse claude response parsing', () => {
  it('parses direct JSON payload', () => {
    const parsed = parseClaudeAssistantPayload('{"text":"hello","operations":[]}')
    expect(parsed).toEqual({ text: 'hello', operations: [] })
  })

  it('parses fenced JSON payload', () => {
    const parsed = parseClaudeAssistantPayload('```json\n{"text":"hello","operations":[]}\n```')
    expect(parsed).toEqual({ text: 'hello', operations: [] })
  })

  it('parses embedded JSON payload in explanatory text', () => {
    const parsed = parseClaudeAssistantPayload(
      'Here is the response:\n{"text":"clean","operations":[{"type":"append_markdown","markdown":"x"}]}',
    )
    expect(parsed?.text).toBe('clean')
    expect(Array.isArray(parsed?.operations)).toBe(true)
  })

  it('returns safe fallback text for unreadable pure-json-shaped output', () => {
    const fallback = fallbackAssistantText('{"text":')
    expect(fallback).toBe('Assistant response was structured but unreadable. Please retry.')
  })
})
