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
    <div
      role="tablist"
      aria-label="Workspace pane"
      className="inline-flex items-center gap-1 rounded-lg border border-[var(--border-subtle)] bg-[var(--surface-base)] p-1"
    >
      <button
        type="button"
        role="tab"
        aria-selected={mobilePane === 'chat'}
        aria-label="Chat pane"
        onClick={() => onMobilePaneChange('chat')}
        className={`inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs font-medium transition-all duration-200 ${
          mobilePane === 'chat'
            ? 'bg-[var(--axon-primary)] text-[var(--axon-bg)] shadow-[var(--shadow-sm)]'
            : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
        }`}
      >
        <MessageSquare className="size-3.5" />
        <span>Chat</span>
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={mobilePane === 'editor'}
        aria-label="Editor pane"
        onClick={() => onMobilePaneChange('editor')}
        className={`inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs font-medium transition-all duration-200 ${
          mobilePane === 'editor'
            ? 'bg-[var(--axon-secondary)] text-[var(--axon-bg)] shadow-[var(--shadow-sm)]'
            : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
        }`}
      >
        <PenLine className="size-3.5" />
        <span>Edit</span>
      </button>
    </div>
  )
}
