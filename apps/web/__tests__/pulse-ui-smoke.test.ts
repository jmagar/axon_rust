import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { PulseChatPane } from '@/components/pulse/pulse-chat-pane'
import { PulseToolbar } from '@/components/pulse/pulse-toolbar'
import type { ChatMessage } from '@/components/pulse/pulse-workspace'
import { detectPulsePromptIntent } from '@/lib/pulse/prompt-intent'

describe('pulse UI smoke workflow', () => {
  it('covers source intent parsing and compact toolbar render', () => {
    const sourceIntent = detectPulsePromptIntent('+source https://example.com/docs')
    expect(sourceIntent.kind).toBe('source')

    const chatIntent = detectPulsePromptIntent('summarize the latest crawl output')
    expect(chatIntent).toEqual({
      kind: 'chat',
      prompt: 'summarize the latest crawl output',
    })

    const toolbarMarkup = renderToStaticMarkup(
      createElement(PulseToolbar, {
        title: 'Untitled',
        onTitleChange: vi.fn(),
      }),
    )

    expect(toolbarMarkup).toContain('pulse-document-title')
    expect(toolbarMarkup).not.toContain('Model selector')
    expect(toolbarMarkup).not.toContain('Permission selector')
  })

  it('renders chat controls for source management and recovery actions', () => {
    const messages: ChatMessage[] = [
      {
        id: 'm1',
        role: 'user',
        content: 'summarize crawl output',
      },
      {
        id: 'm2',
        role: 'assistant',
        content: 'Pulse chat failed (502)',
        isError: true,
        retryPrompt: 'summarize crawl output',
      },
    ]

    const markup = renderToStaticMarkup(
      createElement(PulseChatPane, {
        messages,
        isLoading: false,
        indexedSources: ['https://example.com/docs', 'https://example.com/blog'],
        activeThreadSources: ['https://example.com/docs'],
        onRemoveSource: vi.fn(),
        onRetry: vi.fn(),
        mobilePane: 'chat' as const,
        onMobilePaneChange: vi.fn(),
        isDesktop: true,
      }),
    )

    expect(markup).toContain('Pulse Chat')
    expect(markup).toContain('1 src')
    expect(markup).toContain('Retry')
    expect(markup).toContain('Copy')
  })
})
