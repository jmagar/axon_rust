'use client'

import { ArrowLeftRight, Columns2, FileText, MessageSquare } from 'lucide-react'

type DesktopViewMode = 'chat' | 'editor' | 'both'
type DesktopPaneOrder = 'editor-first' | 'chat-first'

interface PulseToolbarProps {
  title: string
  onTitleChange: (title: string) => void
  isDesktop: boolean
  desktopViewMode: DesktopViewMode
  onDesktopViewModeChange: (mode: DesktopViewMode) => void
  desktopPaneOrder: DesktopPaneOrder
  onSwapPanes: () => void
  contextCharsTotal: number
  contextBudgetChars: number
}

function ContextBar({ used, budget }: { used: number; budget: number }) {
  if (budget <= 0) return null
  const pct = Math.min(100, (used / budget) * 100)
  const color =
    pct >= 90 ? 'bg-rose-500' : pct >= 70 ? 'bg-amber-400' : 'bg-[var(--axon-accent-blue)]'
  const label = `${Math.round(pct)}% context used (${(used / 1000).toFixed(1)}k / ${(budget / 1000).toFixed(0)}k chars)`
  return (
    <div title={label} className="flex items-center gap-1.5">
      <div className="h-1 w-16 overflow-hidden rounded-full bg-[rgba(255,255,255,0.08)]">
        <div
          className={`h-full rounded-full transition-all duration-500 ${color}`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-[length:var(--text-2xs)] text-[var(--axon-text-dim)]">
        {Math.round(pct)}%
      </span>
    </div>
  )
}

export function PulseToolbar({
  title,
  onTitleChange,
  isDesktop,
  desktopViewMode,
  onDesktopViewModeChange,
  desktopPaneOrder,
  onSwapPanes,
  contextCharsTotal,
  contextBudgetChars,
}: PulseToolbarProps) {
  return (
    <div className="flex items-center gap-x-[var(--pulse-control-gap)] rounded-lg border border-[rgba(255,135,175,0.08)] bg-[rgba(10,18,35,0.32)] px-[var(--space-2)] py-[var(--space-2)]">
      <input
        id="pulse-document-title"
        name="pulse_document_title"
        value={title}
        onChange={(e) => onTitleChange(e.target.value)}
        className="min-w-0 flex-1 rounded-md border border-transparent bg-transparent px-[var(--space-2)] py-[var(--pulse-pill-pad-y)] text-[length:var(--text-md)] font-medium text-[var(--axon-text-primary)] outline-none placeholder:text-[var(--axon-text-subtle)] focus:border-[rgba(175,215,255,0.35)] focus:bg-[rgba(10,18,35,0.35)] sm:flex-none sm:w-[40ch]"
        placeholder="Document title..."
      />

      <ContextBar used={contextCharsTotal} budget={contextBudgetChars} />

      {isDesktop && (
        <div className="ml-auto flex items-center gap-1">
          {/* View mode: chat only */}
          <button
            type="button"
            onClick={() => onDesktopViewModeChange('chat')}
            aria-pressed={desktopViewMode === 'chat'}
            title="Chat only"
            className={`inline-flex size-6 items-center justify-center rounded border transition-colors ${
              desktopViewMode === 'chat'
                ? 'border-[rgba(175,215,255,0.35)] bg-[rgba(175,215,255,0.18)] text-[var(--axon-accent-pink-strong)]'
                : 'border-[rgba(255,135,175,0.16)] bg-[rgba(10,18,35,0.42)] text-[var(--axon-text-dim)] hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-text-secondary)]'
            }`}
          >
            <MessageSquare className="size-3" />
          </button>

          {/* View mode: both */}
          <button
            type="button"
            onClick={() => onDesktopViewModeChange('both')}
            aria-pressed={desktopViewMode === 'both'}
            title="Both panes"
            className={`inline-flex size-6 items-center justify-center rounded border transition-colors ${
              desktopViewMode === 'both'
                ? 'border-[rgba(175,215,255,0.35)] bg-[rgba(175,215,255,0.18)] text-[var(--axon-accent-pink-strong)]'
                : 'border-[rgba(255,135,175,0.16)] bg-[rgba(10,18,35,0.42)] text-[var(--axon-text-dim)] hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-text-secondary)]'
            }`}
          >
            <Columns2 className="size-3" />
          </button>

          {/* View mode: editor only */}
          <button
            type="button"
            onClick={() => onDesktopViewModeChange('editor')}
            aria-pressed={desktopViewMode === 'editor'}
            title="Editor only"
            className={`inline-flex size-6 items-center justify-center rounded border transition-colors ${
              desktopViewMode === 'editor'
                ? 'border-[rgba(175,215,255,0.35)] bg-[rgba(175,215,255,0.18)] text-[var(--axon-accent-pink-strong)]'
                : 'border-[rgba(255,135,175,0.16)] bg-[rgba(10,18,35,0.42)] text-[var(--axon-text-dim)] hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-text-secondary)]'
            }`}
          >
            <FileText className="size-3" />
          </button>

          {/* Swap panes — only relevant in both mode */}
          {desktopViewMode === 'both' && (
            <>
              <span className="mx-0.5 h-4 w-px bg-[rgba(255,135,175,0.16)]" />
              <button
                type="button"
                onClick={onSwapPanes}
                title={
                  desktopPaneOrder === 'editor-first'
                    ? 'Chat left, editor right'
                    : 'Editor left, chat right'
                }
                className="inline-flex size-6 items-center justify-center rounded border border-[rgba(255,135,175,0.16)] bg-[rgba(10,18,35,0.42)] text-[var(--axon-text-dim)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-text-secondary)]"
              >
                <ArrowLeftRight className="size-3" />
              </button>
            </>
          )}
        </div>
      )}
    </div>
  )
}
