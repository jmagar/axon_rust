import { describe, expect, it } from 'vitest'
import { buildPulseSystemPrompt } from '@/lib/pulse/rag'

// Defaults for the fields added after these tests were originally written.
const BASE_REQUEST_EXTRAS = {
  disableSlashCommands: false,
  noSessionPersistence: false,
  fallbackModel: '',
  allowedTools: '',
  disallowedTools: '',
  agent: 'claude',
} as const

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
        effort: 'medium',
        maxTurns: 0,
        maxBudgetUsd: 0,
        appendSystemPrompt: '',
        ...BASE_REQUEST_EXTRAS,
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
        effort: 'medium',
        maxTurns: 0,
        maxBudgetUsd: 0,
        appendSystemPrompt: '',
        ...BASE_REQUEST_EXTRAS,
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

  it('includes prior conversation turns in the system prompt', () => {
    const prompt = buildPulseSystemPrompt(
      {
        prompt: 'summarize',
        documentMarkdown: '# Doc',
        selectedCollections: ['pulse'],
        threadSources: [],
        conversationHistory: [
          { role: 'user', content: 'First user turn' },
          { role: 'assistant', content: 'First assistant turn' },
        ],
        permissionLevel: 'accept-edits',
        model: 'sonnet',
        effort: 'medium',
        maxTurns: 0,
        maxBudgetUsd: 0,
        appendSystemPrompt: '',
        ...BASE_REQUEST_EXTRAS,
      },
      [],
    )
    expect(prompt).toContain(
      'Conversation history (oldest to newest, excluding the latest user message):',
    )
    expect(prompt).toContain('User: First user turn')
    expect(prompt).toContain('Assistant: First assistant turn')
  })

  it('bounds conversation history to recent turns', () => {
    const history = Array.from({ length: 30 }, (_, index) => ({
      role: (index % 2 === 0 ? 'user' : 'assistant') as 'user' | 'assistant',
      content: `turn-${index}`,
    }))
    const prompt = buildPulseSystemPrompt(
      {
        prompt: 'summarize',
        documentMarkdown: '# Doc',
        selectedCollections: ['pulse'],
        threadSources: [],
        conversationHistory: history,
        permissionLevel: 'accept-edits',
        model: 'sonnet',
        effort: 'medium',
        maxTurns: 0,
        maxBudgetUsd: 0,
        appendSystemPrompt: '',
        ...BASE_REQUEST_EXTRAS,
      },
      [],
    )

    expect(prompt).not.toContain('turn-0')
    expect(prompt).toContain('turn-29')
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
        effort: 'medium',
        maxTurns: 0,
        maxBudgetUsd: 0,
        appendSystemPrompt: '',
        ...BASE_REQUEST_EXTRAS,
      },
      [],
    )
    expect(prompt.length).toBeLessThan(8000)
  })
})
