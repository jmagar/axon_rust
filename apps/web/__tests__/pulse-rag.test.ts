import { describe, expect, it } from 'vitest'
import { buildPulseSystemPrompt } from '@/lib/pulse/rag'

describe('pulse rag prompt builder', () => {
  it('includes permission level', () => {
    const prompt = buildPulseSystemPrompt(
      {
        prompt: 'summarize',
        documentMarkdown: '# Doc',
        selectedCollections: ['pulse'],
        conversationHistory: [],
        permissionLevel: 'training-wheels',
      },
      [],
    )
    expect(prompt).toContain('Permission level: training-wheels')
  })

  it('includes citation snippets when provided', () => {
    const prompt = buildPulseSystemPrompt(
      {
        prompt: 'summarize',
        documentMarkdown: '# Doc',
        selectedCollections: ['pulse'],
        conversationHistory: [],
        permissionLevel: 'plan',
      },
      [
        {
          url: 'https://example.com',
          title: 'Example',
          snippet: 'Evidence text',
          collection: 'pulse',
          score: 0.9,
        },
      ],
    )
    expect(prompt).toContain('Evidence text')
  })

  it('truncates oversized document context', () => {
    const prompt = buildPulseSystemPrompt(
      {
        prompt: 'summarize',
        documentMarkdown: 'A'.repeat(5000),
        selectedCollections: ['pulse'],
        conversationHistory: [],
        permissionLevel: 'full-access',
      },
      [],
    )
    expect(prompt.length).toBeLessThan(8000)
  })
})
