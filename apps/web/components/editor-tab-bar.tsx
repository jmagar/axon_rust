'use client'

import { Plus, X } from 'lucide-react'
import type { EditorTab } from '@/hooks/use-tabs'

interface EditorTabBarProps {
  tabs: EditorTab[]
  activeTabId: string
  saveStatus: 'idle' | 'saving' | 'saved' | 'error'
  onActivate: (id: string) => void
  onClose: (id: string) => void
  onNewTab: () => void
}

export function EditorTabBar({
  tabs,
  activeTabId,
  saveStatus,
  onActivate,
  onClose,
  onNewTab,
}: EditorTabBarProps) {
  return (
    <div
      className="flex shrink-0 items-center gap-0 overflow-x-auto border-b border-[var(--border-subtle)]"
      style={{ background: 'rgba(10,18,35,0.72)', backdropFilter: 'blur(8px)', minHeight: 36 }}
    >
      {tabs.map((tab) => {
        const isActive = tab.id === activeTabId
        const isDirty = isActive && saveStatus === 'saving'
        const isError = isActive && saveStatus === 'error'

        return (
          <div
            key={tab.id}
            role="tab"
            aria-selected={isActive}
            onClick={() => onActivate(tab.id)}
            className={`group relative flex shrink-0 cursor-pointer select-none items-center gap-1.5 border-r border-[var(--border-subtle)] px-3 py-2 text-[11px] transition-colors ${
              isActive
                ? 'bg-[var(--surface-elevated)] text-[var(--text-primary)]'
                : 'text-[var(--text-dim)] hover:bg-[var(--surface-float)] hover:text-[var(--text-muted)]'
            }`}
            style={{ maxWidth: 200 }}
          >
            {/* Active indicator bar */}
            {isActive && (
              <span
                className="pointer-events-none absolute inset-x-0 top-0 h-[2px] rounded-b-sm"
                style={{ background: 'var(--axon-primary)' }}
              />
            )}

            {/* Dirty / error dot */}
            {(isDirty || isError) && (
              <span
                className="size-1.5 shrink-0 rounded-full"
                style={{
                  background: isError ? 'var(--axon-secondary)' : 'var(--axon-primary)',
                  opacity: 0.8,
                }}
              />
            )}

            <span className="truncate font-mono">{tab.title || 'Untitled'}</span>

            {/* Close button */}
            <button
              type="button"
              aria-label={`Close ${tab.title || 'Untitled'}`}
              title={`Close ${tab.title || 'Untitled'}`}
              onClick={(e) => {
                e.stopPropagation()
                onClose(tab.id)
              }}
              className={`ml-0.5 shrink-0 rounded p-0.5 transition-opacity ${
                isActive
                  ? 'opacity-50 hover:opacity-100'
                  : 'opacity-0 group-hover:opacity-40 hover:!opacity-80'
              }`}
            >
              <X className="size-3" />
            </button>
          </div>
        )
      })}

      {/* New tab button */}
      <button
        type="button"
        aria-label="New tab"
        title="New tab"
        onClick={onNewTab}
        className="flex shrink-0 items-center justify-center px-2.5 py-2 text-[var(--text-dim)] transition-colors hover:bg-[var(--surface-float)] hover:text-[var(--text-muted)]"
      >
        <Plus className="size-3.5" />
      </button>
    </div>
  )
}
