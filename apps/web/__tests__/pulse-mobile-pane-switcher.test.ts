import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { PulseMobilePaneSwitcher } from '@/components/pulse/pulse-mobile-pane-switcher'

describe('pulse mobile pane switcher', () => {
  it('marks chat tab selected when chat is active', () => {
    const markup = renderToStaticMarkup(
      createElement(PulseMobilePaneSwitcher, {
        mobilePane: 'chat' as const,
        onMobilePaneChange: vi.fn(),
      }),
    )

    expect(markup).toContain('aria-label="Chat pane"')
    expect(markup).toContain('aria-label="Editor pane"')
    expect(markup).toContain('aria-selected="true"')
  })

  it('marks editor tab selected when editor is active', () => {
    const markup = renderToStaticMarkup(
      createElement(PulseMobilePaneSwitcher, {
        mobilePane: 'editor' as const,
        onMobilePaneChange: vi.fn(),
      }),
    )

    expect(markup).toContain('aria-selected="false"')
    expect(markup).toContain('aria-selected="true"')
  })
})
