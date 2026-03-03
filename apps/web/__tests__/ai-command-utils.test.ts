import type { UIMessage } from 'ai'
import { describe, expect, it } from 'vitest'
import {
  buildStructuredPrompt,
  formatTextFromMessages,
  getLastUserInstruction,
  getTextFromMessage,
  inlineTag,
  list,
  sections,
  tag,
} from '@/app/api/ai/command/utils'

function makeMessage(role: 'user' | 'assistant', text: string): UIMessage {
  return {
    id: `msg-${Math.random()}`,
    role,
    parts: [{ type: 'text' as const, text }],
    createdAt: new Date(),
  }
}

describe('tag', () => {
  it('wraps content in XML-style tags with newlines', () => {
    expect(tag('tools', 'content')).toBe('<tools>\ncontent\n</tools>')
  })

  it('returns empty string for null content', () => {
    expect(tag('tools', null)).toBe('')
  })

  it('returns empty string for undefined content', () => {
    expect(tag('tools', undefined)).toBe('')
  })

  it('returns empty string for empty string content', () => {
    expect(tag('tools', '')).toBe('')
  })
})

describe('inlineTag', () => {
  it('wraps content inline without newlines', () => {
    expect(inlineTag('code', 'x = 1')).toBe('<code>x = 1</code>')
  })

  it('returns empty string for falsy content', () => {
    expect(inlineTag('code', null)).toBe('')
    expect(inlineTag('code', '')).toBe('')
  })
})

describe('sections', () => {
  it('joins truthy sections with double newlines', () => {
    expect(sections(['a', 'b', 'c'])).toBe('a\n\nb\n\nc')
  })

  it('filters out falsy values', () => {
    expect(sections(['a', false, null, undefined, '', 'b'])).toBe('a\n\nb')
  })

  it('returns empty string when all items are falsy', () => {
    expect(sections([false, null, undefined, ''])).toBe('')
  })
})

describe('list', () => {
  it('formats items as markdown bullet list', () => {
    expect(list(['alpha', 'beta'])).toBe('- alpha\n- beta')
  })

  it('filters empty strings from items', () => {
    expect(list(['a', '', 'b'])).toBe('- a\n- b')
  })

  it('returns empty string for undefined input', () => {
    expect(list(undefined)).toBe('')
  })

  it('returns empty string for empty array', () => {
    expect(list([])).toBe('')
  })
})

describe('getTextFromMessage', () => {
  it('extracts text from message parts', () => {
    const msg = makeMessage('user', 'Hello world')
    expect(getTextFromMessage(msg)).toBe('Hello world')
  })

  it('joins multiple text parts', () => {
    const msg: UIMessage = {
      id: '1',
      role: 'user',
      parts: [
        { type: 'text' as const, text: 'Hello ' },
        { type: 'text' as const, text: 'world' },
      ],
      createdAt: new Date(),
    }
    expect(getTextFromMessage(msg)).toBe('Hello world')
  })
})

describe('formatTextFromMessages', () => {
  it('returns empty string for empty array', () => {
    expect(formatTextFromMessages([])).toBe('')
  })

  it('returns empty string for single message (no history needed)', () => {
    expect(formatTextFromMessages([makeMessage('user', 'hi')])).toBe('')
  })

  it('formats multiple messages as ROLE: text', () => {
    const msgs = [makeMessage('user', 'hello'), makeMessage('assistant', 'hi')]
    const result = formatTextFromMessages(msgs)
    expect(result).toBe('USER: hello\nASSISTANT: hi')
  })

  it('skips messages with empty text', () => {
    const msgs = [
      makeMessage('user', 'hello'),
      makeMessage('assistant', ''),
      makeMessage('user', 'bye'),
    ]
    const result = formatTextFromMessages(msgs)
    expect(result).toBe('USER: hello\nUSER: bye')
  })

  it('respects limit option', () => {
    const msgs = [
      makeMessage('user', 'first'),
      makeMessage('assistant', 'second'),
      makeMessage('user', 'third'),
    ]
    const result = formatTextFromMessages(msgs, { limit: 2 })
    expect(result).toBe('ASSISTANT: second\nUSER: third')
  })
})

describe('getLastUserInstruction', () => {
  it('returns empty string for empty array', () => {
    expect(getLastUserInstruction([])).toBe('')
  })

  it('returns last user message text', () => {
    const msgs = [
      makeMessage('user', 'first'),
      makeMessage('assistant', 'reply'),
      makeMessage('user', 'second'),
    ]
    expect(getLastUserInstruction(msgs)).toBe('second')
  })

  it('returns empty string when no user messages', () => {
    expect(getLastUserInstruction([makeMessage('assistant', 'only assistant')])).toBe('')
  })

  it('trims whitespace', () => {
    expect(getLastUserInstruction([makeMessage('user', '  spaced  ')])).toBe('spaced')
  })
})

describe('buildStructuredPrompt', () => {
  it('includes task in tags', () => {
    const result = buildStructuredPrompt({ task: 'Do something' })
    expect(result).toContain('<task>')
    expect(result).toContain('Do something')
    expect(result).toContain('</task>')
  })

  it('includes instruction with wrapper text', () => {
    const result = buildStructuredPrompt({ instruction: 'Edit this' })
    expect(result).toContain("user's instruction")
    expect(result).toContain('<instruction>')
    expect(result).toContain('Edit this')
  })

  it('includes context with wrapper text', () => {
    const result = buildStructuredPrompt({ context: 'Some context' })
    expect(result).toContain('context you should reference')
    expect(result).toContain('Some context')
  })

  it('includes rules in tags', () => {
    const result = buildStructuredPrompt({ rules: 'Be concise' })
    expect(result).toContain('<rules>')
    expect(result).toContain('Be concise')
  })

  it('formats array examples with example tags', () => {
    const result = buildStructuredPrompt({ examples: ['Example 1', 'Example 2'] })
    expect(result).toContain('<example>')
    expect(result).toContain('Example 1')
    expect(result).toContain('Example 2')
  })

  it('omits sections that are undefined', () => {
    const result = buildStructuredPrompt({ task: 'Only task' })
    expect(result).not.toContain('<rules>')
    expect(result).not.toContain('<context>')
    expect(result).not.toContain('<instruction>')
  })

  it('returns empty string when all sections are undefined', () => {
    expect(buildStructuredPrompt({})).toBe('')
  })
})
