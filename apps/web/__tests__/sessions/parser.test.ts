import { describe, expect, it } from 'vitest'
import { parseClaudeJsonl } from '@/lib/sessions/claude-jsonl-parser'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Serialise a JSONL line with a user string message. */
function userLine(content: string): string {
  return JSON.stringify({ type: 'user', message: { content } })
}

/** Serialise a JSONL line with an assistant string message. */
function assistantLine(content: string): string {
  return JSON.stringify({ type: 'assistant', message: { content } })
}

/** Serialise a JSONL line with array-style content blocks. */
function userArrayLine(blocks: Array<{ type: string; text?: string }>): string {
  return JSON.stringify({ type: 'user', message: { content: blocks } })
}

// ---------------------------------------------------------------------------
// parseClaudeJsonl
// ---------------------------------------------------------------------------

describe('parseClaudeJsonl', () => {
  it('returns [] for empty string', () => {
    expect(parseClaudeJsonl('')).toEqual([])
  })

  it('returns [] for whitespace-only string', () => {
    expect(parseClaudeJsonl('   \n\n  \t  ')).toEqual([])
  })

  it('skips invalid JSON lines', () => {
    const raw = ['not json at all', userLine('Valid message'), '{broken json'].join('\n')

    const result = parseClaudeJsonl(raw)
    expect(result).toHaveLength(1)
    expect(result[0]).toEqual({ role: 'user', content: 'Valid message' })
  })

  it('skips non-user/assistant type lines', () => {
    const raw = [
      JSON.stringify({ type: 'system', message: { content: 'System prompt' } }),
      JSON.stringify({ type: 'tool_result', message: { content: 'Tool output' } }),
      userLine('Real user message'),
    ].join('\n')

    const result = parseClaudeJsonl(raw)
    expect(result).toHaveLength(1)
    expect(result[0]).toEqual({ role: 'user', content: 'Real user message' })
  })

  it('extracts string content from user and assistant messages', () => {
    const raw = [userLine('Hello world'), assistantLine('Hi there!')].join('\n')

    const result = parseClaudeJsonl(raw)
    expect(result).toHaveLength(2)
    expect(result[0]).toEqual({ role: 'user', content: 'Hello world' })
    expect(result[1]).toEqual({ role: 'assistant', content: 'Hi there!' })
  })

  it('extracts text from array content blocks', () => {
    const raw = userArrayLine([
      { type: 'text', text: 'First block. ' },
      { type: 'image' }, // no text field — should be skipped
      { type: 'text', text: 'Second block.' },
    ])

    const result = parseClaudeJsonl(raw)
    expect(result).toHaveLength(1)
    expect(result[0]!.content).toContain('First block.')
    expect(result[0]!.content).toContain('Second block.')
  })

  it('skips lines with array content that contains no text blocks', () => {
    const raw = userArrayLine([{ type: 'image' }, { type: 'tool_use' }])
    // No text → content is empty after trim → should not push a message
    const result = parseClaudeJsonl(raw)
    expect(result).toHaveLength(0)
  })

  it('skips messages where content is neither string nor array', () => {
    const raw = JSON.stringify({ type: 'user', message: { content: 42 } })
    const result = parseClaudeJsonl(raw)
    expect(result).toHaveLength(0)
  })

  it('skips lines exceeding MAX_LINE_BYTES (512 KB)', () => {
    // Build a line slightly over 512 KB
    const HALF_MB = 512_001
    const bigValue = 'x'.repeat(HALF_MB)
    const bigLine = JSON.stringify({ type: 'user', message: { content: bigValue } })
    // The serialised line will be well over 512 KB
    expect(bigLine.length).toBeGreaterThan(512_000)

    const raw = [bigLine, userLine('Small valid line')].join('\n')
    const result = parseClaudeJsonl(raw)

    // Only the small line should survive
    expect(result).toHaveLength(1)
    expect(result[0]).toEqual({ role: 'user', content: 'Small valid line' })
  })

  it('strips null bytes from input before parsing', () => {
    // A bare null byte between two valid lines: '\0' on its own line would be
    // a syntax-error line if not stripped, but after stripping it becomes an
    // empty line which is skipped.
    const raw = `${userLine('Before')}\n\0\n${userLine('After')}`

    const result = parseClaudeJsonl(raw)
    // The lone \0 line becomes empty after stripping → skipped.
    expect(result.length).toBe(2)
    const contents = result.map((m) => m.content)
    expect(contents).toContain('Before')
    expect(contents).toContain('After')
  })

  it('does not treat a JSON-encoded null escape (\\u0000) as a raw null byte', () => {
    // JSON.stringify encodes \0 as the escape \u0000 in the text.
    // The module strips raw \0 bytes from the input, not JSON escape sequences.
    // After JSON.parse the decoded string value will still contain a null char —
    // this is expected and out of scope for the input-layer stripping.
    const lineWithNullEscape = userLine('HelloWorld')
    const raw = lineWithNullEscape
    const result = parseClaudeJsonl(raw)
    expect(result).toHaveLength(1)
    expect(result[0]!.content).toBe('HelloWorld')
  })

  it('does not throw on very long input (> 1 MB)', () => {
    // Generate > 1 MB of input: many valid small lines
    const line = userLine('Some normal user input here')
    const count = Math.ceil(1_100_000 / (line.length + 1))
    const raw = Array.from({ length: count }, () => line).join('\n')

    expect(raw.length).toBeGreaterThan(1_000_000)
    expect(() => parseClaudeJsonl(raw)).not.toThrow()
    // All lines are valid and small, so all should parse
    const result = parseClaudeJsonl(raw)
    expect(result.length).toBe(count)
  })

  it('handles multiple blank lines gracefully', () => {
    const raw = `\n\n${userLine('Only message')}\n\n\n`
    const result = parseClaudeJsonl(raw)
    expect(result).toHaveLength(1)
    expect(result[0]).toEqual({ role: 'user', content: 'Only message' })
  })
})
