import { describe, expect, it } from 'vitest'
import { computeContextCharsTotal } from '@/app/api/pulse/chat/claude-stream-types'

describe('computeContextCharsTotal', () => {
  const base = {
    globalClaudeMdChars: 0,
    systemPromptChars: 0,
    promptLength: 0,
    documentMarkdownLength: 0,
    citationSnippets: [] as string[],
    threadSources: [] as string[],
    conversationHistory: [] as Array<{ content: string }>,
  }

  it('returns 0 when all inputs are zero/empty', () => {
    expect(computeContextCharsTotal(base)).toBe(0)
  })

  it('sums all scalar fields', () => {
    expect(
      computeContextCharsTotal({
        ...base,
        globalClaudeMdChars: 100,
        systemPromptChars: 200,
        promptLength: 50,
        documentMarkdownLength: 300,
      }),
    ).toBe(650)
  })

  it('sums citation snippet lengths', () => {
    expect(
      computeContextCharsTotal({
        ...base,
        citationSnippets: ['abc', 'de'],
      }),
    ).toBe(5)
  })

  it('sums thread source lengths', () => {
    expect(
      computeContextCharsTotal({
        ...base,
        threadSources: ['source1', 'ab'],
      }),
    ).toBe(9)
  })

  it('sums conversation history content lengths', () => {
    expect(
      computeContextCharsTotal({
        ...base,
        conversationHistory: [{ content: 'hello' }, { content: 'world!' }],
      }),
    ).toBe(11)
  })

  it('sums everything together', () => {
    const total = computeContextCharsTotal({
      globalClaudeMdChars: 1000,
      systemPromptChars: 500,
      promptLength: 100,
      documentMarkdownLength: 2000,
      citationSnippets: ['aaa', 'bb'],
      threadSources: ['x'],
      conversationHistory: [{ content: 'msg1' }, { content: 'msg2' }],
    })
    // 1000 + 500 + 100 + 2000 + 5 + 1 + 8 = 3614
    expect(total).toBe(3614)
  })
})
