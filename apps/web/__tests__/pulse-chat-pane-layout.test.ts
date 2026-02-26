import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { PulseChatPane, computeMessageVirtualWindow } from '@/components/pulse/pulse-chat-pane'
import type { ChatMessage } from '@/components/pulse/pulse-workspace'

describe('pulse chat virtualization', () => {
  it('does not virtualize short conversations', () => {
    expect(computeMessageVirtualWindow(12, 0, 640)).toEqual({
      shouldVirtualize: false,
      start: 0,
      end: 12,
    })
  })

  it('virtualizes long conversations for mobile viewport', () => {
    const windowState = computeMessageVirtualWindow(260, 2400, 640)
    expect(windowState.shouldVirtualize).toBe(true)
    expect(windowState.start).toBeGreaterThan(0)
    expect(windowState.end).toBeLessThanOrEqual(260)
    expect(windowState.end).toBeGreaterThan(windowState.start)
  })
})

describe('pulse chat pane snapshots', () => {
  it('renders compact empty-state header', () => {
    const markup = renderToStaticMarkup(
      createElement(PulseChatPane, {
        messages: [],
        isLoading: false,
        indexedSources: [],
        activeThreadSources: [],
        onRemoveSource: vi.fn(),
        onRetry: vi.fn(),
        mobilePane: 'chat' as const,
        onMobilePaneChange: vi.fn(),
        isDesktop: true,
      }),
    )

    expect(markup).toMatchSnapshot()
  })

  it('renders notice + sources state', () => {
    const messages: ChatMessage[] = [
      {
        id: 'u1',
        role: 'user',
        content: 'Summarize this crawl.',
        createdAt: 1700000000000,
      },
      {
        id: 'a1',
        role: 'assistant',
        content: 'Working through indexed context.',
        createdAt: 1700000005000,
        citations: [
          {
            title: 'Tailwind CSS v4 docs',
            url: 'https://tailwindcss.com/docs',
            snippet: 'Theme variables and @theme usage.',
            collection: 'cortex',
            score: 0.91,
          },
        ],
      },
    ]

    const markup = renderToStaticMarkup(
      createElement(PulseChatPane, {
        messages,
        isLoading: true,
        indexedSources: ['https://tailwindcss.com/docs'],
        activeThreadSources: ['https://tailwindcss.com/docs'],
        onRemoveSource: vi.fn(),
        onRetry: vi.fn(),
        mobilePane: 'chat' as const,
        onMobilePaneChange: vi.fn(),
        isDesktop: false,
        requestNotice: 'Previous request replaced by your latest prompt.',
      }),
    )

    expect(markup).toMatchSnapshot()
  })
})
