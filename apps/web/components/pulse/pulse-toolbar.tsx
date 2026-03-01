'use client'

import { Bot, Network, Plus, Settings2 } from 'lucide-react'
import { useRouter } from 'next/navigation'
import { useState } from 'react'

interface PulseToolbarProps {
  title: string
  onTitleChange: (title: string) => void
  isDesktop?: boolean
  onNewSession?: () => void
}

export function PulseToolbar({
  title,
  onTitleChange,
  isDesktop = false,
  onNewSession,
}: PulseToolbarProps) {
  const router = useRouter()
  const [isDirty, setIsDirty] = useState(false)
  return (
    <div className="flex items-center gap-x-[var(--pulse-control-gap)] rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.32)] px-[var(--space-2)] py-[var(--space-2)]">
      <div className="relative flex min-w-0 flex-1 sm:flex-none sm:w-[40ch]">
        <input
          id="pulse-document-title"
          name="pulse_document_title"
          value={title}
          onChange={(e) => {
            onTitleChange(e.target.value)
            setIsDirty(true)
          }}
          className="w-full rounded-md border border-transparent bg-transparent px-[var(--space-2)] py-[var(--pulse-pill-pad-y)] text-[length:var(--text-md)] font-medium text-[var(--text-primary)] outline-none placeholder:text-[var(--text-dim)] focus:border-[var(--border-standard)] focus:bg-[var(--surface-elevated)]"
          placeholder="Document title..."
        />
        {isDirty && (
          <span
            className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 size-1.5 rounded-full bg-[var(--axon-secondary)] animate-pulse"
            title="Unsaved changes"
          />
        )}
      </div>

      {isDesktop && (
        <div className="ml-auto flex items-center gap-1">
          {/* New session — separator + button */}
          {onNewSession && (
            <>
              <span className="mx-0.5 h-4 w-px bg-[var(--border-subtle)]" />
              <button
                type="button"
                onClick={onNewSession}
                title="New session — clears chat and document"
                className="ui-chip inline-flex items-center gap-1 rounded border border-[rgba(95,135,175,0.24)] bg-[rgba(10,18,35,0.45)] px-1.5 py-0.5 text-[var(--text-dim)] transition-colors hover:border-[rgba(175,215,255,0.35)] hover:text-[var(--text-secondary)]"
              >
                <Plus className="size-3" />
                <span className="text-[10px] font-medium leading-none">New</span>
              </button>
            </>
          )}

          {/* Nav — MCP, Agents, Settings */}
          <span className="mx-0.5 h-4 w-px bg-[var(--border-subtle)]" />
          <button
            type="button"
            onClick={() => router.push('/mcp')}
            title="MCP Servers"
            aria-label="MCP Servers"
            className="inline-flex size-6 items-center justify-center rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.42)] text-[var(--text-dim)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-primary-strong)]"
          >
            <Network className="size-3" />
          </button>
          <button
            type="button"
            onClick={() => router.push('/agents')}
            title="Available Agents"
            aria-label="Available Agents"
            className="inline-flex size-6 items-center justify-center rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.42)] text-[var(--text-dim)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-primary-strong)]"
          >
            <Bot className="size-3" />
          </button>
          <button
            type="button"
            onClick={() => router.push('/settings')}
            title="Settings"
            aria-label="Open settings"
            className="inline-flex size-6 items-center justify-center rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.42)] text-[var(--text-dim)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-primary-strong)]"
          >
            <Settings2 className="size-3" />
          </button>
        </div>
      )}
    </div>
  )
}
