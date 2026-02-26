import { describe, expect, it } from 'vitest'
import { buildPulseSystemPrompt } from '@/lib/pulse/rag'

describe('pulse rag prompt builder', () => {
  it('includes permission level', () => {
    const prompt = buildPulseSystemPrompt(
      {
        prompt: 'summarize',
        documentMarkdown: '# Doc',
        selectedCollections: ['pulse'],
        threadSources: [],
        conversationHistory: [],
        permissionLevel: 'accept-edits',
        model: 'sonnet',
      },
      [],
    )
    expect(prompt).toContain('Permission level: accept-edits')
  })

  it('includes citation snippets when provided', () => {
    const prompt = buildPulseSystemPrompt(
      {
        prompt: 'summarize',
        documentMarkdown: '# Doc',
        selectedCollections: ['pulse'],
        threadSources: [],
        conversationHistory: [],
        permissionLevel: 'plan',
        model: 'sonnet',
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
        threadSources: [],
        conversationHistory: [],
        permissionLevel: 'bypass-permissions',
        model: 'sonnet',
      },
      [],
    )
    expect(prompt.length).toBeLessThan(8000)
  })
})
