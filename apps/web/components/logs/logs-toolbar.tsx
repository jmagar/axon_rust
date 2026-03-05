'use client'

import { Pause, Play, Trash2, WrapText } from 'lucide-react'

export const SERVICES = [
  'axon-workers',
  'axon-web',
  'axon-postgres',
  'axon-redis',
  'axon-rabbitmq',
  'axon-qdrant',
  'axon-chrome',
] as const

export type IndividualService = (typeof SERVICES)[number]
export type ServiceName = IndividualService | 'all'

export const TAIL_OPTIONS = [50, 100, 200, 500, 1000] as const
export type TailLines = (typeof TAIL_OPTIONS)[number]

interface LogsToolbarProps {
  service: ServiceName
  tailLines: TailLines
  filter: string
  autoScroll: boolean
  compact: boolean
  wrapLines: boolean
  isConnected: boolean
  onServiceChange: (s: ServiceName) => void
  onTailChange: (t: TailLines) => void
  onFilterChange: (f: string) => void
  onAutoScrollToggle: () => void
  onCompactToggle: () => void
  onWrapToggle: () => void
  onClear: () => void
}

const SELECT_CLASS =
  'rounded-md border px-2 py-1 text-[11px] font-medium focus:outline-none focus:ring-1 focus:ring-[var(--axon-primary)] transition-colors cursor-pointer'
const SELECT_STYLE = {
  background: 'rgba(10,18,35,0.7)',
  borderColor: 'var(--border-subtle)',
  color: 'var(--text-secondary)',
}

export function LogsToolbar({
  service,
  tailLines,
  filter,
  autoScroll,
  compact,
  wrapLines,
  isConnected,
  onServiceChange,
  onTailChange,
  onFilterChange,
  onAutoScrollToggle,
  onCompactToggle,
  onWrapToggle,
  onClear,
}: LogsToolbarProps) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      {/* Service selector */}
      <div className="flex items-center gap-1.5">
        <span className="text-[10px] font-semibold uppercase tracking-widest text-[var(--text-dim)]">
          Service
        </span>
        <select
          className={SELECT_CLASS}
          style={SELECT_STYLE}
          value={service}
          onChange={(e) => onServiceChange(e.target.value as ServiceName)}
          aria-label="Select service"
        >
          <option value="all">All services</option>
          {SERVICES.map((s) => (
            <option key={s} value={s}>
              {s}
            </option>
          ))}
        </select>
      </div>

      {/* Tail selector */}
      <div className="flex items-center gap-1.5">
        <span className="text-[10px] font-semibold uppercase tracking-widest text-[var(--text-dim)]">
          Tail
        </span>
        <select
          className={SELECT_CLASS}
          style={SELECT_STYLE}
          value={tailLines}
          onChange={(e) => onTailChange(Number(e.target.value) as TailLines)}
          aria-label="Select tail lines"
        >
          {TAIL_OPTIONS.map((n) => (
            <option key={n} value={n}>
              {n}
            </option>
          ))}
        </select>
      </div>

      {/* Filter input */}
      <input
        type="text"
        placeholder="Filter logs…"
        value={filter}
        onChange={(e) => onFilterChange(e.target.value)}
        className="min-w-[140px] rounded-md border px-2 py-1 text-[11px] placeholder:text-[var(--text-dim)] focus:outline-none focus:ring-1 focus:ring-[var(--axon-primary)] transition-colors"
        style={{
          background: 'rgba(10,18,35,0.7)',
          borderColor: 'var(--border-subtle)',
          color: 'var(--text-secondary)',
        }}
        aria-label="Filter log lines"
      />

      {/* Auto-scroll toggle */}
      <button
        type="button"
        onClick={onAutoScrollToggle}
        className="flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-[11px] font-medium transition-colors hover:border-[var(--border-standard)] hover:text-[var(--text-primary)]"
        style={{
          background: autoScroll ? 'rgba(135,175,255,0.12)' : 'rgba(10,18,35,0.7)',
          borderColor: autoScroll ? 'var(--border-standard)' : 'var(--border-subtle)',
          color: autoScroll ? 'var(--axon-primary)' : 'var(--text-dim)',
        }}
        aria-pressed={autoScroll}
        title={autoScroll ? 'Pause auto-scroll' : 'Resume auto-scroll'}
      >
        {autoScroll ? <Pause className="size-3" /> : <Play className="size-3" />}
        Auto-scroll
      </button>

      <button
        type="button"
        onClick={onCompactToggle}
        className="rounded-md border px-2.5 py-1 text-[11px] font-medium transition-colors hover:border-[var(--border-standard)] hover:text-[var(--text-primary)]"
        style={{
          background: compact ? 'rgba(135,175,255,0.12)' : 'rgba(10,18,35,0.7)',
          borderColor: compact ? 'var(--border-standard)' : 'var(--border-subtle)',
          color: compact ? 'var(--axon-primary)' : 'var(--text-dim)',
        }}
        aria-pressed={compact}
        title={compact ? 'Switch to comfortable spacing' : 'Switch to compact spacing'}
      >
        Compact
      </button>

      <button
        type="button"
        onClick={onWrapToggle}
        className="flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-[11px] font-medium transition-colors hover:border-[var(--border-standard)] hover:text-[var(--text-primary)]"
        style={{
          background: wrapLines ? 'rgba(135,175,255,0.12)' : 'rgba(10,18,35,0.7)',
          borderColor: wrapLines ? 'var(--border-standard)' : 'var(--border-subtle)',
          color: wrapLines ? 'var(--axon-primary)' : 'var(--text-dim)',
        }}
        aria-pressed={wrapLines}
        title={wrapLines ? 'Disable line wrapping' : 'Enable line wrapping'}
      >
        <WrapText className="size-3" />
        Wrap
      </button>

      <button
        type="button"
        onClick={onClear}
        className="flex items-center gap-1.5 rounded-md border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.7)] px-2.5 py-1 text-[11px] font-medium text-[var(--text-dim)] transition-colors hover:border-[var(--border-standard)] hover:text-[var(--text-primary)]"
        title="Clear visible log buffer"
      >
        <Trash2 className="size-3" />
        Clear
      </button>

      {/* Connection status */}
      <div className="ml-auto flex items-center gap-1.5">
        <span
          className="inline-block size-2 rounded-full"
          style={{
            background: isConnected ? 'var(--axon-success)' : '#ef4444',
            boxShadow: isConnected
              ? '0 0 6px rgba(130,217,160,0.6)'
              : '0 0 6px rgba(239,68,68,0.6)',
          }}
          aria-hidden="true"
        />
        <span className="text-[10px] font-medium text-[var(--text-dim)]">
          {isConnected ? 'Live' : 'Disconnected'}
        </span>
      </div>
    </div>
  )
}
