'use client'

import {
  AlertCircle,
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  Ban,
  CheckCircle2,
  Clock,
  ExternalLink,
  RotateCcw,
} from 'lucide-react'
import Link from 'next/link'
import { useState } from 'react'
import type { Job, JobStatus, JobType, StatusCounts } from '@/app/api/jobs/route'

// ── Helpers ───────────────────────────────────────────────────────────────────

// 3. RELATIVE TIMESTAMPS with absolute on hover
export function formatRelativeTime(isoString: string | null): {
  relative: string
  absolute: string
} {
  if (!isoString) return { relative: '—', absolute: '—' }
  const date = new Date(isoString)
  const diff = Date.now() - date.getTime()
  const absolute = date.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
  if (diff < 60_000) return { relative: 'just now', absolute }
  if (diff < 3_600_000) return { relative: `${Math.floor(diff / 60_000)}m ago`, absolute }
  if (diff < 86_400_000) return { relative: `${Math.floor(diff / 3_600_000)}h ago`, absolute }
  return { relative: `${Math.floor(diff / 86_400_000)}d ago`, absolute }
}

// 4. SMART URL TRUNCATION — show last 2 path segments
export function smartTruncate(target: string): string {
  if (target.startsWith('http')) {
    try {
      const url = new URL(target)
      const parts = url.pathname.split('/').filter(Boolean)
      if (parts.length >= 2) return `…/${parts.slice(-2).join('/')}`
      if (parts.length === 1) return `${url.hostname}/…/${parts[0]}`
      return url.hostname
    } catch {
      // fall through to path logic
    }
  }
  const parts = target.split(/[/\\]/).filter(Boolean)
  if (parts.length >= 2) return `…/${parts.slice(-2).join('/')}`
  return target
}

// ── 1. COLOR-CODED TYPE CHIP ─────────────────────────────────────────────────

const TYPE_STYLES: Record<JobType, { chip: string; dot: string }> = {
  crawl: {
    chip: 'text-[#38bdf8] bg-[rgba(56,189,248,0.1)] border border-[rgba(56,189,248,0.25)]',
    dot: '#38bdf8',
  },
  embed: {
    chip: 'text-[#fbbf24] bg-[rgba(251,191,36,0.1)] border border-[rgba(251,191,36,0.25)]',
    dot: '#fbbf24',
  },
  extract: {
    chip: 'text-[#a78bfa] bg-[rgba(167,139,250,0.1)] border border-[rgba(167,139,250,0.25)]',
    dot: '#a78bfa',
  },
  ingest: {
    chip: 'text-[#fb7185] bg-[rgba(251,113,133,0.1)] border border-[rgba(251,113,133,0.25)]',
    dot: '#fb7185',
  },
}

export function TypeChip({ type }: { type: JobType }) {
  const s = TYPE_STYLES[type]
  return (
    <span
      className={`inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-widest ${s.chip}`}
    >
      <span className="size-1.5 flex-shrink-0 rounded-full" style={{ background: s.dot }} />
      {type}
    </span>
  )
}

// ── 2. RICHER STATUS BADGE ────────────────────────────────────────────────────

export function StatusBadge({ status }: { status: JobStatus }) {
  if (status === 'running') {
    return (
      <span className="inline-flex items-center gap-2">
        <span className="relative flex size-2.5 flex-shrink-0">
          <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[#38bdf8] opacity-60" />
          <span className="relative inline-flex size-2.5 rounded-full bg-[#38bdf8]" />
        </span>
        <span className="text-[10px] font-semibold text-[#38bdf8]">Active</span>
        {/* 8. RUNNING PROGRESS — indeterminate shimmer bar */}
        <span className="h-1 w-10 overflow-hidden rounded-full bg-[rgba(56,189,248,0.15)]">
          <span className="block h-full w-full animate-shimmer bg-[linear-gradient(90deg,transparent_25%,rgba(56,189,248,0.7)_50%,transparent_75%)] [background-size:200%_100%]" />
        </span>
      </span>
    )
  }
  if (status === 'pending') {
    return (
      <span className="inline-flex items-center gap-1.5">
        <Clock className="size-3 text-[var(--text-dim)]" />
        <span className="text-[10px] font-medium text-[var(--text-dim)]">Pending</span>
      </span>
    )
  }
  if (status === 'completed') {
    return (
      <span className="inline-flex items-center gap-1.5 opacity-40">
        <CheckCircle2 className="size-3 text-[var(--axon-success)]" />
        <span className="text-[10px] font-medium text-[var(--axon-success)]">Done</span>
      </span>
    )
  }
  if (status === 'failed') {
    return (
      <span className="inline-flex items-center gap-1.5">
        <AlertCircle className="size-3 text-red-400" />
        <span className="text-[10px] font-semibold text-red-400">Failed</span>
      </span>
    )
  }
  return (
    <span className="inline-flex items-center gap-1.5 opacity-60">
      <Ban className="size-3 text-yellow-400" />
      <span className="text-[10px] font-medium text-yellow-400">Canceled</span>
    </span>
  )
}

// ── 6. STATS SUMMARY BAR ──────────────────────────────────────────────────────

function StatPip({
  color,
  label,
  count,
  pulse,
}: {
  color: string
  label: string
  count: number
  pulse?: boolean
}) {
  return (
    <span
      className={`flex items-center gap-1.5 text-[10px] transition-opacity ${count === 0 ? 'opacity-30' : ''}`}
    >
      <span
        className={`size-1.5 flex-shrink-0 rounded-full ${pulse && count > 0 ? 'animate-pulse' : ''}`}
        style={{ background: color }}
      />
      <span className="font-mono font-semibold tabular-nums" style={{ color }}>
        {count.toLocaleString()}
      </span>
      <span className="text-[var(--text-dim)]">{label}</span>
    </span>
  )
}

export function StatsBar({ counts }: { counts: StatusCounts | undefined }) {
  if (!counts) return null
  const total = counts.running + counts.pending + counts.completed + counts.failed
  if (total === 0) return null
  return (
    <div className="mb-3 flex flex-wrap items-center gap-5 rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.4)] px-4 py-2.5">
      <StatPip color="#38bdf8" label="active" count={counts.running} pulse />
      <StatPip color="var(--text-dim)" label="pending" count={counts.pending} />
      <StatPip color="var(--axon-success)" label="done" count={counts.completed} />
      <StatPip color="#f87171" label="failed" count={counts.failed} />
    </div>
  )
}

// ── 7. SORTABLE COLUMN HEADER ─────────────────────────────────────────────────

export type SortColumn = 'type' | 'target' | 'collection' | 'status' | 'started'
export type SortDir = 'asc' | 'desc'

export function SortableHeader({
  column,
  label,
  sort,
  onSort,
}: {
  column: SortColumn
  label: string
  sort: { column: SortColumn; dir: SortDir }
  onSort: (col: SortColumn) => void
}) {
  const active = sort.column === column
  return (
    <button
      type="button"
      onClick={() => onSort(column)}
      className={[
        'flex items-center gap-1 text-[10px] font-semibold uppercase tracking-widest transition-colors',
        'hover:text-[var(--text-secondary)]',
        active ? 'text-[var(--axon-primary)]' : 'text-[var(--text-dim)]',
      ].join(' ')}
    >
      {label}
      {active ? (
        sort.dir === 'asc' ? (
          <ArrowUp className="size-3" />
        ) : (
          <ArrowDown className="size-3" />
        )
      ) : (
        <ArrowUpDown className="size-3 opacity-30" />
      )}
    </button>
  )
}

// ── Skeleton row ──────────────────────────────────────────────────────────────

export function SkeletonRow() {
  const shimmer =
    'animate-shimmer rounded bg-[linear-gradient(90deg,rgba(135,175,255,0.04)_25%,rgba(135,175,255,0.09)_50%,rgba(135,175,255,0.04)_75%)] [background-size:200%_100%]'
  return (
    <tr className="border-b border-[var(--border-subtle)]">
      <td className="px-3 py-2.5">
        <div className={`${shimmer} h-3 w-14`} />
      </td>
      <td className="px-3 py-2.5">
        <div className={`${shimmer} h-3 w-full max-w-[280px]`} />
      </td>
      <td className="px-3 py-2.5">
        <div className={`${shimmer} h-3 w-16`} />
      </td>
      <td className="px-3 py-2.5">
        <div className={`${shimmer} h-4 w-20 rounded-full`} />
      </td>
      <td className="px-3 py-2.5">
        <div className={`${shimmer} h-3 w-16`} />
      </td>
      <td className="px-3 py-2.5">
        <div className={`${shimmer} h-5 w-14 rounded`} />
      </td>
    </tr>
  )
}

// ── 5. JOB ROW with hover actions ────────────────────────────────────────────

export function JobRow({
  job,
  onCancel,
}: {
  job: Job
  onCancel: (id: string, type: JobType) => void
}) {
  const [hovered, setHovered] = useState(false)
  const canCancel = job.status === 'pending' || job.status === 'running'
  const { relative, absolute } = formatRelativeTime(job.startedAt)
  const truncated = smartTruncate(job.target)

  return (
    <tr
      className="group border-b border-[var(--border-subtle)] transition-colors duration-100 hover:bg-[rgba(135,175,255,0.04)]"
      title={job.errorText ?? undefined}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <td className="px-3 py-2.5">
        <TypeChip type={job.type} />
      </td>
      <td className="max-w-[280px] px-3 py-2.5">
        <Link
          href={`/jobs/${job.id}`}
          className="group/link flex min-w-0 items-center gap-1.5"
          title={job.target}
        >
          <span className="block truncate font-mono text-[11px] text-[var(--text-secondary)] transition-colors group-hover/link:text-[var(--axon-primary)]">
            {truncated}
          </span>
          <ExternalLink className="size-3 flex-shrink-0 text-[var(--text-dim)] opacity-0 transition-opacity group-hover/link:opacity-100" />
        </Link>
      </td>
      <td className="px-3 py-2.5">
        {job.collection ? (
          <span className="whitespace-nowrap rounded bg-[rgba(135,175,255,0.07)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--text-dim)]">
            {job.collection}
          </span>
        ) : (
          <span className="text-[10px] text-[var(--text-dim)]">—</span>
        )}
      </td>
      <td className="px-3 py-2.5">
        <StatusBadge status={job.status} />
      </td>
      <td
        className="whitespace-nowrap px-3 py-2.5 font-mono text-[10px] text-[var(--text-dim)]"
        title={absolute}
      >
        {relative}
      </td>
      <td className="w-20 px-3 py-2.5">
        <div
          className={`flex items-center gap-0.5 transition-opacity duration-150 ${hovered ? 'opacity-100' : 'opacity-0'}`}
        >
          {canCancel && (
            <button
              type="button"
              onClick={() => onCancel(job.id, job.type)}
              className="rounded p-1 text-[var(--text-dim)] transition-colors hover:bg-[rgba(255,135,175,0.12)] hover:text-[var(--axon-secondary)] focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
              title="Cancel job"
              aria-label="Cancel job"
            >
              <Ban className="size-3.5" />
            </button>
          )}
          {job.status === 'failed' && (
            <button
              type="button"
              className="rounded p-1 text-[var(--text-dim)] transition-colors hover:bg-[rgba(251,191,36,0.12)] hover:text-[#fbbf24] focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
              title="Retry (not yet supported)"
              aria-label="Retry job"
            >
              <RotateCcw className="size-3.5" />
            </button>
          )}
          <Link
            href={`/jobs/${job.id}`}
            className="rounded p-1 text-[var(--text-dim)] transition-colors hover:bg-[rgba(135,175,255,0.1)] hover:text-[var(--axon-primary)]"
            title="View details"
          >
            <ExternalLink className="size-3.5" />
          </Link>
        </div>
      </td>
    </tr>
  )
}
