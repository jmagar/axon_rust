import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

// PulseToolbar uses useRouter() which requires the Next.js App Router context.
// Provide a minimal stub so renderToStaticMarkup doesn't throw.
vi.mock('next/navigation', () => ({
  useRouter: () => ({ push: vi.fn(), back: vi.fn(), forward: vi.fn(), replace: vi.fn() }),
  usePathname: () => '/',
  useSearchParams: () => new URLSearchParams(),
}))

import { PulseChatPane } from '@/components/pulse/pulse-chat-pane'
import { PulseToolbar } from '@/components/pulse/pulse-toolbar'
import { detectPulsePromptIntent } from '@/lib/pulse/prompt-intent'
import type { ChatMessage } from '@/lib/pulse/workspace-persistence'

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
        sourcesExpanded: false,
        onSourcesExpandedChange: vi.fn(),
      }),
    )

    // "Pulse Chat" header was removed when the pane was refactored to a pure content area.
    // "1 src" count moved to the workspace toolbar — not rendered inside PulseChatPane.
    expect(markup).toContain('Retry')
    expect(markup).toContain('Copy')
  })
})
