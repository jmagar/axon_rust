import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

vi.mock('@/hooks/use-axon-ws', () => ({
  useAxonWs: () => ({
    send: vi.fn(),
    subscribe: () => () => {},
  }),
}))

vi.mock('@/hooks/use-ws-messages', () => ({
  useWsMessages: () => ({
    startExecution: vi.fn(),
    activateWorkspace: vi.fn(),
    submitWorkspacePrompt: vi.fn(),
    currentJobId: null,
    currentMode: 'scrape',
    workspaceMode: 'pulse',
    workspacePromptVersion: 1,
    workspacePrompt: null,
    workspaceContext: {
      turns: 3,
      sourceCount: 2,
      threadSourceCount: 1,
      promptChars: 120,
      documentChars: 400,
      conversationChars: 800,
      citationChars: 50,
      contextCharsTotal: 1370,
      contextBudgetChars: 120000,
      lastLatencyMs: 920,
      model: 'sonnet',
      permissionLevel: 'accept-edits',
      saveStatus: 'saved',
    },
    pulseModel: 'sonnet',
    pulsePermissionLevel: 'accept-edits',
    setPulseModel: vi.fn(),
    setPulsePermissionLevel: vi.fn(),
  }),
}))

import { Omnibox } from '@/components/omnibox'

describe('omnibox visual snapshots', () => {
  it('renders stable pulse controls', () => {
    const markup = renderToStaticMarkup(createElement(Omnibox))
    expect(markup).toMatchSnapshot()
  })
})
