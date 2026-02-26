import { describe, expect, it } from 'vitest'
import { detectPulsePromptIntent } from '@/lib/pulse/prompt-intent'

describe('pulse prompt intent', () => {
  it('treats a standalone URL as a source-ingest request', () => {
    const intent = detectPulsePromptIntent('https://nginx.org/en/docs/')
    expect(intent.kind).toBe('source')
    if (intent.kind === 'source') {
      expect(intent.urls).toEqual(['https://nginx.org/en/docs/'])
    }
  })

  it('treats add-source style command with URL as source-ingest request', () => {
    const intent = detectPulsePromptIntent('add source https://example.com/docs')
    expect(intent.kind).toBe('source')
  })

  it('treats +source command with URL as source-ingest request', () => {
    const intent = detectPulsePromptIntent('+source https://example.com/docs')
    expect(intent.kind).toBe('source')
  })

  it('treats normal questions as chat prompts', () => {
    const intent = detectPulsePromptIntent('can you research more about nginx reverse proxies?')
    expect(intent).toEqual({
      kind: 'chat',
      prompt: 'can you research more about nginx reverse proxies?',
    })
  })

  it('does not hijack mixed prompt+URL questions into source mode', () => {
    const intent = detectPulsePromptIntent('what does this page mean? https://example.com/docs')
    expect(intent.kind).toBe('chat')
  })
})
