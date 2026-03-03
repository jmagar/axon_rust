import { describe, expect, it } from 'vitest'
import {
  createStreamParserState,
  extractToolResultText,
  parseClaudeStreamLine,
} from '@/app/api/pulse/chat/stream-parser'

describe('createStreamParserState', () => {
  it('returns a fresh state', () => {
    const state = createStreamParserState()
    expect(state.blocks).toEqual([])
    expect(state.toolUseIdToIdx.size).toBe(0)
    expect(state.toolUses).toEqual([])
    expect(state.result).toBe('')
    expect(state.sessionId).toBeNull()
    expect(state.firstDeltaMs).toBeNull()
    expect(state.deltaCount).toBe(0)
  })
})

describe('extractToolResultText', () => {
  it('returns string input directly', () => {
    expect(extractToolResultText('hello')).toBe('hello')
  })

  it('returns empty string for null/undefined', () => {
    expect(extractToolResultText(null)).toBe('')
    expect(extractToolResultText(undefined)).toBe('')
  })

  it('returns empty string for non-array objects', () => {
    expect(extractToolResultText({ text: 'nope' })).toBe('')
  })

  it('extracts text from array of {text} objects', () => {
    const input = [{ text: 'line 1' }, { text: 'line 2' }]
    expect(extractToolResultText(input)).toBe('line 1\nline 2')
  })

  it('extracts nested content arrays', () => {
    const input = [{ content: [{ text: 'inner' }] }]
    expect(extractToolResultText(input)).toBe('inner')
  })

  it('skips non-object entries in the array', () => {
    const input = ['bare string', null, 42, { text: 'valid' }]
    expect(extractToolResultText(input)).toBe('valid')
  })

  it('handles empty array', () => {
    expect(extractToolResultText([])).toBe('')
  })

  it('ignores entries without text or content', () => {
    expect(extractToolResultText([{ other: 'field' }])).toBe('')
  })
})

describe('parseClaudeStreamLine', () => {
  const now = Date.now()

  it('skips empty lines', () => {
    const state = createStreamParserState()
    expect(parseClaudeStreamLine('', state, now)).toEqual({ kind: 'skip' })
    expect(parseClaudeStreamLine('   ', state, now)).toEqual({ kind: 'skip' })
  })

  it('skips invalid JSON', () => {
    const state = createStreamParserState()
    expect(parseClaudeStreamLine('not json', state, now)).toEqual({ kind: 'skip' })
  })

  it('skips unknown event types', () => {
    const state = createStreamParserState()
    const line = JSON.stringify({ type: 'system', subtype: 'init' })
    expect(parseClaudeStreamLine(line, state, now)).toEqual({ kind: 'skip' })
  })

  it('parses assistant text block', () => {
    const state = createStreamParserState()
    const line = JSON.stringify({
      type: 'assistant',
      message: { content: [{ type: 'text', text: 'Hello' }] },
    })
    const result = parseClaudeStreamLine(line, state, now)
    expect(result.kind).toBe('assistant_events')
    if (result.kind === 'assistant_events') {
      const deltas = result.events.filter((e) => e.type === 'assistant_delta')
      expect(deltas).toHaveLength(1)
      expect(deltas[0]).toEqual({ type: 'assistant_delta', delta: 'Hello' })
    }
    expect(state.blocks).toHaveLength(1)
    expect(state.blocks[0]).toEqual({ type: 'text', content: 'Hello' })
    expect(state.deltaCount).toBe(1)
    expect(state.firstDeltaMs).toBeTypeOf('number')
  })

  it('parses assistant tool_use block', () => {
    const state = createStreamParserState()
    const line = JSON.stringify({
      type: 'assistant',
      message: {
        content: [{ type: 'tool_use', id: 'tu-1', name: 'Read', input: { path: '/foo' } }],
      },
    })
    const result = parseClaudeStreamLine(line, state, now)
    expect(result.kind).toBe('assistant_events')
    if (result.kind === 'assistant_events') {
      const tools = result.events.filter((e) => e.type === 'tool_use')
      expect(tools).toHaveLength(1)
    }
    expect(state.toolUses).toHaveLength(1)
    expect(state.toolUses[0].name).toBe('Read')
    expect(state.toolUseIdToIdx.get('tu-1')).toBe(0)
  })

  it('updates tool_use in-place on duplicate ID', () => {
    const state = createStreamParserState()
    // First tool_use
    parseClaudeStreamLine(
      JSON.stringify({
        type: 'assistant',
        message: {
          content: [{ type: 'tool_use', id: 'tu-1', name: 'Read', input: {} }],
        },
      }),
      state,
      now,
    )
    // Second tool_use with same ID but updated input
    parseClaudeStreamLine(
      JSON.stringify({
        type: 'assistant',
        message: {
          content: [{ type: 'tool_use', id: 'tu-1', name: 'Read', input: { path: '/bar' } }],
        },
      }),
      state,
      now,
    )
    // Should still have one block, updated in-place
    expect(state.blocks).toHaveLength(1)
    expect((state.blocks[0] as { input: Record<string, unknown> }).input).toEqual({ path: '/bar' })
    expect(state.toolUses).toHaveLength(1)
    expect(state.toolUses[0].input).toEqual({ path: '/bar' })
  })

  it('parses thinking block', () => {
    const state = createStreamParserState()
    const line = JSON.stringify({
      type: 'assistant',
      message: { content: [{ type: 'thinking', thinking: 'Let me think...' }] },
    })
    const result = parseClaudeStreamLine(line, state, now)
    expect(result.kind).toBe('assistant_events')
    if (result.kind === 'assistant_events') {
      const thinking = result.events.filter((e) => e.type === 'thinking_content')
      expect(thinking).toHaveLength(1)
    }
    expect(state.blocks).toHaveLength(1)
    expect(state.blocks[0]).toEqual({ type: 'thinking', content: 'Let me think...' })
  })

  it('updates thinking block in-place on consecutive thinking events', () => {
    const state = createStreamParserState()
    parseClaudeStreamLine(
      JSON.stringify({
        type: 'assistant',
        message: { content: [{ type: 'thinking', thinking: 'First' }] },
      }),
      state,
      now,
    )
    parseClaudeStreamLine(
      JSON.stringify({
        type: 'assistant',
        message: { content: [{ type: 'thinking', thinking: 'Updated thought' }] },
      }),
      state,
      now,
    )
    expect(state.blocks).toHaveLength(1)
    expect(state.blocks[0]).toEqual({ type: 'thinking', content: 'Updated thought' })
  })

  it('parses tool_result and patches block', () => {
    const state = createStreamParserState()
    // First, add the tool_use
    parseClaudeStreamLine(
      JSON.stringify({
        type: 'assistant',
        message: {
          content: [{ type: 'tool_use', id: 'tu-1', name: 'Read', input: {} }],
        },
      }),
      state,
      now,
    )
    // Then the result
    const result = parseClaudeStreamLine(
      JSON.stringify({
        type: 'tool_result',
        tool_use_id: 'tu-1',
        content: [{ text: 'file contents' }],
      }),
      state,
      now,
    )
    expect(result).toEqual({
      kind: 'tool_result_patch',
      toolUseId: 'tu-1',
      resultText: 'file contents',
    })
    // Block should have result patched
    const block = state.blocks[0] as { result?: string }
    expect(block.result).toBe('file contents')
  })

  it('tool_result truncates result to 600 chars', () => {
    const state = createStreamParserState()
    parseClaudeStreamLine(
      JSON.stringify({
        type: 'assistant',
        message: {
          content: [{ type: 'tool_use', id: 'tu-1', name: 'Read', input: {} }],
        },
      }),
      state,
      now,
    )
    const longText = 'x'.repeat(1000)
    parseClaudeStreamLine(
      JSON.stringify({
        type: 'tool_result',
        tool_use_id: 'tu-1',
        content: [{ text: longText }],
      }),
      state,
      now,
    )
    const block = state.blocks[0] as { result?: string }
    expect(block.result).toHaveLength(600)
  })

  it('tool_result skips when no matching tool_use_id', () => {
    const state = createStreamParserState()
    const result = parseClaudeStreamLine(
      JSON.stringify({
        type: 'tool_result',
        tool_use_id: 'unknown',
        content: [{ text: 'orphan' }],
      }),
      state,
      now,
    )
    // Still returns the patch since id and resultText exist
    expect(result.kind).toBe('tool_result_patch')
  })

  it('tool_result skips when no content', () => {
    const state = createStreamParserState()
    const result = parseClaudeStreamLine(
      JSON.stringify({ type: 'tool_result', tool_use_id: 'tu-1' }),
      state,
      now,
    )
    expect(result).toEqual({ kind: 'skip' })
  })

  it('parses result event', () => {
    const state = createStreamParserState()
    const result = parseClaudeStreamLine(
      JSON.stringify({ type: 'result', result: 'success', session_id: 'sess-1' }),
      state,
      now,
    )
    expect(result).toEqual({ kind: 'result', result: 'success' })
    expect(state.result).toBe('success')
    expect(state.sessionId).toBe('sess-1')
  })

  it('result event with no result/session_id', () => {
    const state = createStreamParserState()
    parseClaudeStreamLine(JSON.stringify({ type: 'result' }), state, now)
    expect(state.result).toBe('')
    expect(state.sessionId).toBeNull()
  })
})
