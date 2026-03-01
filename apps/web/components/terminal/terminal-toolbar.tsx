'use client'

import { Search } from 'lucide-react'
import type { WsStatus } from '@/lib/ws-protocol'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface TerminalToolbarProps {
  status: WsStatus
  isRunning: boolean
  onClear: () => void
  onCopy: () => void
  onCancelCurrent: () => void
  searchVisible: boolean
  onToggleSearch: () => void
}

// ---------------------------------------------------------------------------
// Status dot config
// ---------------------------------------------------------------------------

const STATUS_CONFIG: Record<WsStatus, { color: string; label: string }> = {
  connected: { color: '#82d9a0', label: 'CONNECTED' },
  reconnecting: { color: '#ffc086', label: 'RECONNECTING...' },
  disconnected: { color: 'rgba(255,135,175,0.8)', label: 'DISCONNECTED' },
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function TerminalToolbar({
  status,
  isRunning,
  onClear,
  onCopy,
  onCancelCurrent,
  searchVisible,
  onToggleSearch,
}: TerminalToolbarProps) {
  const { color: dotColor, label: statusLabel } = STATUS_CONFIG[status]

  return (
    <div
      className="flex h-10 flex-shrink-0 items-center justify-between px-3"
      style={{
        background: 'rgba(9,18,37,0.85)',
        backdropFilter: 'blur(12px)',
        borderBottom: '1px solid rgba(255,135,175,0.12)',
      }}
    >
      {/* Left: title + status */}
      <div className="flex items-center gap-3">
        <span
          className="select-none font-mono text-[11px] font-semibold tracking-[2px]"
          style={{ color: 'var(--axon-text-muted, #93aaca)' }}
        >
          TERMINAL
        </span>

        <div className="flex items-center gap-1.5">
          {/* Status dot */}
          <span
            className="inline-block size-1.5 rounded-full"
            style={{ background: dotColor, boxShadow: `0 0 4px ${dotColor}` }}
            aria-hidden="true"
          />
          <span className="font-mono text-[10px] tracking-wide" style={{ color: dotColor }}>
            {statusLabel}
          </span>
        </div>

        {/* Spinner when running */}
        {isRunning && (
          <div role="status" aria-label="Command running">
            <div
              className="size-3 animate-spin rounded-full border"
              style={{
                borderColor: 'rgba(135,175,255,0.25)',
                borderTopColor: '#87afff',
              }}
              aria-hidden="true"
            />
          </div>
        )}
      </div>

      {/* Right: action buttons */}
      <div className="flex items-center gap-1">
        {isRunning && (
          <ToolbarButton
            onClick={onCancelCurrent}
            label="CANCEL"
            style={{ color: 'rgba(255,135,175,0.9)' }}
            hoverColor="rgba(255,135,175,1)"
          />
        )}

        <ToolbarButton onClick={onClear} label="CLEAR" />
        <ToolbarButton onClick={onCopy} label="COPY" />

        {/* Search toggle */}
        <button
          type="button"
          onClick={onToggleSearch}
          aria-label="Toggle search"
          title="Toggle search (Ctrl+F)"
          className="flex items-center justify-center rounded px-1.5 py-1 transition-colors"
          style={{
            color: searchVisible
              ? 'var(--axon-accent-blue, #afd7ff)'
              : 'var(--axon-text-muted, #93aaca)',
            background: searchVisible ? 'rgba(135,175,255,0.1)' : 'transparent',
          }}
          onMouseEnter={(e) => {
            if (!searchVisible)
              (e.currentTarget as HTMLButtonElement).style.color =
                'var(--axon-accent-blue, #afd7ff)'
          }}
          onMouseLeave={(e) => {
            if (!searchVisible)
              (e.currentTarget as HTMLButtonElement).style.color = 'var(--axon-text-muted, #93aaca)'
          }}
        >
          <Search className="size-3.5" />
        </button>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface ToolbarButtonProps {
  onClick: () => void
  label: string
  style?: React.CSSProperties
  hoverColor?: string
}

function ToolbarButton({
  onClick,
  label,
  style,
  hoverColor = 'var(--axon-accent-blue, #afd7ff)',
}: ToolbarButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded px-2 py-1 font-mono text-[10px] font-medium tracking-wide transition-colors"
      style={{
        color: 'var(--axon-text-muted, #93aaca)',
        ...style,
      }}
      onMouseEnter={(e) => {
        ;(e.currentTarget as HTMLButtonElement).style.color = hoverColor
      }}
      onMouseLeave={(e) => {
        ;(e.currentTarget as HTMLButtonElement).style.color =
          style?.color ?? 'var(--axon-text-muted, #93aaca)'
      }}
    >
      {label}
    </button>
  )
}
