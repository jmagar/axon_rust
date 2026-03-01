'use client'

import { MessageSquare, PenLine } from 'lucide-react'

interface PulseMobilePaneSwitcherProps {
  mobilePane: 'chat' | 'editor'
  onMobilePaneChange: (pane: 'chat' | 'editor') => void
}

export function PulseMobilePaneSwitcher({
  mobilePane,
  onMobilePaneChange,
}: PulseMobilePaneSwitcherProps) {
  return (
    <div role="tablist" aria-label="Workspace pane" className="inline-flex items-center gap-1">
      <button
        type="button"
        role="tab"
        aria-selected={mobilePane === 'chat'}
        aria-label="Chat pane"
        onClick={() => onMobilePaneChange('chat')}
        className={`inline-flex size-7 items-center justify-center rounded border transition-all duration-200 backdrop-blur-sm ${
          mobilePane === 'chat'
            ? 'border-[rgba(175,215,255,0.25)] bg-[var(--axon-primary)] text-[var(--axon-bg)] shadow-[var(--shadow-sm)]'
            : 'border-[var(--border-subtle)] bg-[rgba(10,18,35,0.42)] text-[var(--text-dim)] hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-primary-strong)]'
        }`}
      >
        <MessageSquare className="size-3.5" />
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={mobilePane === 'editor'}
        aria-label="Editor pane"
        onClick={() => onMobilePaneChange('editor')}
        className={`inline-flex size-7 items-center justify-center rounded border transition-all duration-200 backdrop-blur-sm ${
          mobilePane === 'editor'
            ? 'border-[rgba(255,135,175,0.25)] bg-[var(--axon-secondary)] text-[var(--axon-bg)] shadow-[var(--shadow-sm)]'
            : 'border-[var(--border-subtle)] bg-[rgba(10,18,35,0.42)] text-[var(--text-dim)] hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-primary-strong)]'
        }`}
      >
        <PenLine className="size-3.5" />
      </button>
    </div>
  )
}
