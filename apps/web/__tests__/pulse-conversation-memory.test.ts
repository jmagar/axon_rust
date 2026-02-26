import { describe, expect, it } from 'vitest'
import { resolveConversationMemoryAnswer } from '@/lib/pulse/conversation-memory'

describe('pulse conversation memory fallback', () => {
  it('recalls favorite color from prior user turn', () => {
    const answer = resolveConversationMemoryAnswer('What is my favorite color?', [
      { role: 'user', content: 'My favorite color is blue.' },
      { role: 'assistant', content: 'Nice choice.' },
    ])

    expect(answer).toBe('Your favorite color is blue.')
  })

  it('supports favourite/colour spelling variants', () => {
    const answer = resolveConversationMemoryAnswer('What is my favourite colour?', [
      { role: 'user', content: 'My favourite colour is forest green.' },
      { role: 'assistant', content: 'Got it.' },
    ])

    expect(answer).toBe('Your favorite color is forest green.')
  })

  it('returns not-told message when no color statement exists', () => {
    const answer = resolveConversationMemoryAnswer('What is my favorite color?', [
      { role: 'user', content: 'hello there' },
    ])

    expect(answer).toBe("You haven't told me your favorite color yet.")
  })

  it('does not trigger for unrelated prompts', () => {
    const answer = resolveConversationMemoryAnswer('Summarize this document', [
      { role: 'user', content: 'My favorite color is blue.' },
    ])

    expect(answer).toBeNull()
  })
})
