import { describe, expect, it } from 'vitest'
import { encodeCopilotStreamEvent, parseOpenAiSseChunk } from '@/app/api/ai/copilot/route'
import { validateCopilotRequest } from '@/lib/pulse/copilot-validation'

describe('copilot request validation', () => {
  it('rejects empty prompt', () => {
    expect(validateCopilotRequest({ prompt: '' }).valid).toBe(false)
  })

  it('accepts valid prompt', () => {
    expect(validateCopilotRequest({ prompt: 'Continue: The quick brown' }).valid).toBe(true)
  })

  it('accepts prompt with optional system message', () => {
    const result = validateCopilotRequest({
      prompt: 'Continue this text',
      system: 'You are a writing assistant.',
    })
    expect(result.valid).toBe(true)
  })
})

describe('copilot stream helpers', () => {
  it('encodes NDJSON completion events', () => {
    const line = encodeCopilotStreamEvent({
      type: 'delta',
      delta: 'Hello',
      completion: 'Hello',
    })

    expect(line).toBe('{"type":"delta","delta":"Hello","completion":"Hello"}\n')
  })

  it('parses SSE delta chunks across boundaries', () => {
    const partA = 'data: {"choices":[{"delta":{"content":"Hel'
    const partB = 'lo"}}]}\n\ndata: [DONE]\n\n'

    const first = parseOpenAiSseChunk(partA, '')
    expect(first.deltas).toEqual([])
    expect(first.done).toBe(false)

    const second = parseOpenAiSseChunk(partB, first.remainder)
    expect(second.deltas).toEqual(['Hello'])
    expect(second.done).toBe(true)
    expect(second.remainder).toBe('')
  })

  it('ignores malformed SSE lines', () => {
    const payload = 'data: not-json\n\ndata: {"choices":[{"delta":{"content":"ok"}}]}\n\n'
    const parsed = parseOpenAiSseChunk(payload, '')

    expect(parsed.deltas).toEqual(['ok'])
    expect(parsed.done).toBe(false)
  })

  it('ignores comments/non-data lines and stops at done marker', () => {
    const payload = [
      ': keepalive',
      'event: message',
      'data: {"choices":[{"delta":{"content":"A"}}]}',
      'data: [DONE]',
      'data: {"choices":[{"delta":{"content":"B"}}]}',
      '',
    ].join('\n')

    const parsed = parseOpenAiSseChunk(payload, '')
    expect(parsed.deltas).toEqual(['A'])
    expect(parsed.done).toBe(true)
  })
})
