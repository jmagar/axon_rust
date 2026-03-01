'use client'

import { useVirtualizer } from '@tanstack/react-virtual'
import { useState } from 'react'
import { fmtNum } from './shared'

export type SortDir = 'asc' | 'desc'

export const VIRTUAL_THRESHOLD = 200
export const DISPLAY_LIMIT = 100

// ---------------------------------------------------------------------------
// Top-N toggle
// ---------------------------------------------------------------------------

export function TopNToggle({
  totalRows,
  showAll,
  onToggle,
}: {
  totalRows: number
  showAll: boolean
  onToggle: () => void
}) {
  if (totalRows <= 1000) return null
  return (
    <div className="mb-2 flex items-center justify-between">
      <span className="text-xs text-[var(--text-muted)]">
        {showAll
          ? `All ${totalRows.toLocaleString()} rows`
          : `Top ${DISPLAY_LIMIT} of ${totalRows.toLocaleString()} rows`}
      </span>
      <button
        type="button"
        onClick={onToggle}
        className="text-xs text-[var(--axon-primary)] hover:text-[var(--axon-primary-strong)] transition-colors"
      >
        {showAll ? 'Show top 100' : `Show all ${totalRows.toLocaleString()}`}
      </button>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Filter input
// ---------------------------------------------------------------------------

export function FilterInput({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <input
      type="text"
      placeholder="Filter..."
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="mb-3 w-full rounded-md border border-[var(--border-subtle)] px-3 py-1.5 text-sm text-[var(--text-secondary)] placeholder-[var(--text-dim)] outline-none transition-colors focus:border-[var(--border-accent)]"
      style={{ background: 'var(--surface-base)' }}
    />
  )
}

// ---------------------------------------------------------------------------
// Sortable header
// ---------------------------------------------------------------------------

export function SortHeader({
  label,
  sortKey,
  currentSort,
  currentDir,
  onSort,
  align = 'left',
}: {
  label: string
  sortKey: string
  currentSort: string
  currentDir: SortDir
  onSort: (key: string) => void
  align?: 'left' | 'right'
}) {
  const active = currentSort === sortKey
  return (
    <th
      className={`cursor-pointer select-none ui-table-head transition-colors hover:text-[var(--axon-primary)] ${align === 'right' ? 'text-right' : 'text-left'}`}
      onClick={() => onSort(sortKey)}
    >
      {label}
      {active && (
        <span className="ml-1 text-[var(--axon-primary-strong)]">
          {currentDir === 'asc' ? '\u25B2' : '\u25BC'}
        </span>
      )}
    </th>
  )
}

// ---------------------------------------------------------------------------
// URL cell
// ---------------------------------------------------------------------------

export function UrlCell({ url }: { url: string }) {
  const isAbsolute = url.startsWith('http://') || url.startsWith('https://')
  return isAbsolute ? (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      className="text-[var(--axon-primary-strong)] transition-colors hover:text-[var(--axon-primary)] hover:underline"
    >
      {url}
    </a>
  ) : (
    <span>{url}</span>
  )
}

// ---------------------------------------------------------------------------
// Status badge
// ---------------------------------------------------------------------------

export function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, { color: string; background: string }> = {
    completed: { color: 'var(--axon-success)', background: 'var(--axon-success-bg)' },
    running: { color: 'var(--axon-primary-strong)', background: 'rgba(135,175,255,0.14)' },
    pending: { color: 'var(--axon-warning)', background: 'var(--axon-warning-bg)' },
    failed: { color: 'var(--axon-secondary)', background: 'rgba(255,135,175,0.14)' },
    canceled: { color: 'var(--text-muted)', background: 'rgba(147,170,202,0.14)' },
  }
  const style = colors[status] ?? {
    color: 'var(--text-muted)',
    background: 'rgba(147,170,202,0.14)',
  }
  return (
    <span className="ui-chip-status" style={style}>
      {status}
    </span>
  )
}

// ---------------------------------------------------------------------------
// Virtual table body
// ---------------------------------------------------------------------------

export function VirtualTableBody<T>({
  rows,
  parentRef,
  renderRow,
}: {
  rows: T[]
  parentRef: React.RefObject<HTMLDivElement | null>
  renderRow: (row: T, index: number) => React.ReactNode
}) {
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 10,
  })

  return (
    <tbody
      style={{
        height: `${rowVirtualizer.getTotalSize()}px`,
        position: 'relative',
        display: 'block',
      }}
    >
      {rowVirtualizer.getVirtualItems().map((virtualRow) => (
        <tr
          key={virtualRow.key}
          data-index={virtualRow.index}
          ref={rowVirtualizer.measureElement}
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            width: '100%',
            transform: `translateY(${virtualRow.start}px)`,
            display: 'table-row',
          }}
          className="border-b border-[var(--border-subtle)] hover:bg-[var(--surface-float)]"
        >
          {renderRow(rows[virtualRow.index], virtualRow.index)}
        </tr>
      ))}
    </tbody>
  )
}

// ---------------------------------------------------------------------------
// useShowAll hook
// ---------------------------------------------------------------------------

export function useShowAll() {
  const [showAll, setShowAll] = useState(false)
  return { showAll, toggleShowAll: () => setShowAll((v) => !v) }
}

// ---------------------------------------------------------------------------
// Formatting re-export
// ---------------------------------------------------------------------------

export { fmtNum }
