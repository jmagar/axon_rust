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
  Loader2,
  RefreshCw,
  RotateCcw,
  Zap,
} from 'lucide-react'
import Link from 'next/link'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { Job, JobStatus, JobType, StatusCounts } from '@/app/api/jobs/route'

// ── Types ─────────────────────────────────────────────────────────────────────

type TypeFilter = 'all' | JobType
type StatusFilter = 'all' | 'active' | 'pending' | 'completed' | 'failed'
type SortColumn = 'type' | 'target' | 'collection' | 'status' | 'started'
type SortDir = 'asc' | 'desc'

interface JobsApiResponse {
  jobs: Job[]
  total: number
  hasMore: boolean
  counts?: StatusCounts
  error?: string
}

// ── Constants ─────────────────────────────────────────────────────────────────

const TYPE_TABS: { value: TypeFilter; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'crawl', label: 'Crawl' },
  { value: 'extract', label: 'Extract' },
  { value: 'embed', label: 'Embed' },
  { value: 'ingest', label: 'Ingest' },
]

const STATUS_TABS: { value: StatusFilter; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'active', label: 'Active' },
  { value: 'pending', label: 'Pending' },
  { value: 'completed', label: 'Done' },
  { value: 'failed', label: 'Failed' },
]

const PAGE_SIZE = 50
const POLL_INTERVAL_MS = 3000

// Status sort priority: active > pending > failed > canceled > done
const STATUS_ORDER: Record<JobStatus, number> = {
  running: 0,
  pending: 1,
  failed: 2,
  canceled: 3,
  completed: 4,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// 3. RELATIVE TIMESTAMPS with absolute on hover
function formatRelativeTime(isoString: string | null): { relative: string; absolute: string } {
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
function smartTruncate(target: string): string {
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

// 7. CLIENT-SIDE SORT
function sortJobs(jobs: Job[], column: SortColumn, dir: SortDir): Job[] {
  return [...jobs].sort((a, b) => {
    let cmp = 0
    switch (column) {
      case 'type':
        cmp = a.type.localeCompare(b.type)
        break
      case 'target':
        cmp = a.target.localeCompare(b.target)
        break
      case 'collection':
        cmp = (a.collection ?? '').localeCompare(b.collection ?? '')
        break
      case 'status':
        cmp = STATUS_ORDER[a.status] - STATUS_ORDER[b.status]
        break
      case 'started': {
        const ta = a.startedAt ? new Date(a.startedAt).getTime() : 0
        const tb = b.startedAt ? new Date(b.startedAt).getTime() : 0
        cmp = ta - tb
        break
      }
    }
    return dir === 'asc' ? cmp : -cmp
  })
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

function TypeChip({ type }: { type: JobType }) {
  const s = TYPE_STYLES[type]
  return (
    <span
      className={`inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-widest ${s.chip}`}
    >
      <span className="size-1.5 rounded-full flex-shrink-0" style={{ background: s.dot }} />
      {type}
    </span>
  )
}

// ── 2. RICHER STATUS BADGE ────────────────────────────────────────────────────

function StatusBadge({ status }: { status: JobStatus }) {
  if (status === 'running') {
    return (
      <span className="inline-flex items-center gap-2">
        {/* Animated pulse ring */}
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

  // canceled
  return (
    <span className="inline-flex items-center gap-1.5 opacity-60">
      <Ban className="size-3 text-yellow-400" />
      <span className="text-[10px] font-medium text-yellow-400">Canceled</span>
    </span>
  )
}

// ── Filter pill ───────────────────────────────────────────────────────────────

function FilterPill({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        'rounded-md px-2.5 py-1 text-[10px] font-semibold uppercase tracking-widest transition-all duration-150',
        'focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1',
        active
          ? 'bg-[rgba(135,175,255,0.18)] text-[var(--axon-primary)] shadow-[0_0_8px_rgba(135,175,255,0.15)]'
          : 'text-[var(--text-dim)] hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)]',
      ].join(' ')}
    >
      {children}
    </button>
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
        className={`size-1.5 rounded-full flex-shrink-0 ${pulse && count > 0 ? 'animate-pulse' : ''}`}
        style={{ background: color }}
      />
      <span className="font-mono font-semibold tabular-nums" style={{ color }}>
        {count.toLocaleString()}
      </span>
      <span className="text-[var(--text-dim)]">{label}</span>
    </span>
  )
}

function StatsBar({ counts }: { counts: StatusCounts | undefined }) {
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

function SortableHeader({
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

function SkeletonRow() {
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

function JobRow({ job, onCancel }: { job: Job; onCancel: (id: string, type: JobType) => void }) {
  const [hovered, setHovered] = useState(false)
  const canCancel = job.status === 'pending' || job.status === 'running'

  // 3. RELATIVE TIMESTAMPS
  const { relative, absolute } = formatRelativeTime(job.startedAt)

  // 4. SMART URL TRUNCATION
  const truncated = smartTruncate(job.target)

  return (
    <tr
      className="group border-b border-[var(--border-subtle)] transition-colors duration-100 hover:bg-[rgba(135,175,255,0.04)]"
      title={job.errorText ?? undefined}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {/* 1. TYPE CHIP */}
      <td className="px-3 py-2.5">
        <TypeChip type={job.type} />
      </td>

      {/* 4. URL COLUMN — smart truncation with full path on hover */}
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

      {/* 2. RICHER STATUS */}
      <td className="px-3 py-2.5">
        <StatusBadge status={job.status} />
      </td>

      {/* 3. RELATIVE TIME — absolute datetime in title tooltip */}
      <td
        className="whitespace-nowrap px-3 py-2.5 font-mono text-[10px] text-[var(--text-dim)]"
        title={absolute}
      >
        {relative}
      </td>

      {/* 5. HOVER ACTIONS */}
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

// ── Main dashboard ────────────────────────────────────────────────────────────

export function JobsDashboard() {
  const [typeFilter, setTypeFilter] = useState<TypeFilter>('all')
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all')
  const [jobs, setJobs] = useState<Job[]>([])
  const [total, setTotal] = useState(0)
  const [hasMore, setHasMore] = useState(false)
  const [offset, setOffset] = useState(0)
  const [loading, setLoading] = useState(true)
  const [loadingMore, setLoadingMore] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [spinning, setSpinning] = useState(false)
  const [cancelMsg, setCancelMsg] = useState<string | null>(null)
  const [counts, setCounts] = useState<StatusCounts | undefined>(undefined)
  // 7. SORT STATE
  const [sort, setSort] = useState<{ column: SortColumn; dir: SortDir }>({
    column: 'started',
    dir: 'desc',
  })
  const [tick, setTick] = useState(0)
  const pollRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const hasActiveJobs = jobs.some((j) => j.status === 'pending' || j.status === 'running')

  const filterRef = useRef({ typeFilter, statusFilter, offset })
  filterRef.current = { typeFilter, statusFilter, offset }

  // 7. SORTED JOBS — client-side on loaded batch
  const sortedJobs = useMemo(() => sortJobs(jobs, sort.column, sort.dir), [jobs, sort])

  // biome-ignore lint/correctness/useExhaustiveDependencies: tick is an imperative trigger; filterRef provides latest values without re-subscribing
  useEffect(() => {
    const controller = new AbortController()
    const { typeFilter: type, statusFilter: status } = filterRef.current

    async function run() {
      setLoading(true)
      setError(null)
      try {
        const params = new URLSearchParams({ type, status, limit: String(PAGE_SIZE), offset: '0' })
        const res = await fetch(`/api/jobs?${params}`, { signal: controller.signal })
        const data = (await res.json()) as JobsApiResponse
        if (data.error) {
          setError(data.error)
          return
        }
        setJobs(data.jobs)
        setOffset(0)
        setTotal(data.total)
        setHasMore(data.hasMore)
        if (data.counts) setCounts(data.counts)
      } catch (err) {
        if (err instanceof Error && err.name === 'AbortError') return
        setError(err instanceof Error ? err.message : 'Failed to fetch jobs')
      } finally {
        setLoading(false)
        setSpinning(false)
      }
    }

    void run()
    return () => controller.abort()
  }, [tick])

  // biome-ignore lint/correctness/useExhaustiveDependencies: tick included to reset poll timer after each fetch cycle
  useEffect(() => {
    if (!hasActiveJobs) return
    pollRef.current = setTimeout(() => setTick((t) => t + 1), POLL_INTERVAL_MS)
    return () => {
      if (pollRef.current) clearTimeout(pollRef.current)
    }
  }, [hasActiveJobs, tick])

  function handleRefresh() {
    setSpinning(true)
    setOffset(0)
    setTick((t) => t + 1)
  }

  function handleLoadMore() {
    const next = offset + PAGE_SIZE
    setOffset(next)
    setLoadingMore(true)
    const { typeFilter: type, statusFilter: status } = filterRef.current
    const params = new URLSearchParams({
      type,
      status,
      limit: String(PAGE_SIZE),
      offset: String(next),
    })
    fetch(`/api/jobs?${params}`)
      .then((r) => r.json())
      .then((data: JobsApiResponse) => {
        if (data.error) {
          setError(data.error)
          return
        }
        setJobs((prev) => [...prev, ...data.jobs])
        setTotal(data.total)
        setHasMore(data.hasMore)
      })
      .catch((err: unknown) => {
        setError(err instanceof Error ? err.message : 'Failed to fetch jobs')
      })
      .finally(() => setLoadingMore(false))
  }

  function handleCancel(_id: string, _type: JobType) {
    setCancelMsg('Cancel not yet supported from UI')
    setTimeout(() => setCancelMsg(null), 3000)
  }

  function handleSort(column: SortColumn) {
    setSort((prev) =>
      prev.column === column
        ? { column, dir: prev.dir === 'asc' ? 'desc' : 'asc' }
        : { column, dir: 'desc' },
    )
  }

  return (
    <div className="mx-auto max-w-6xl p-6 animate-fade-in">
      {/* Header */}
      <div className="mb-6 flex items-center gap-3">
        <div className="flex items-center gap-2">
          <Zap className="size-4 text-[var(--axon-primary)]" />
          <h1 className="text-[18px] font-bold tracking-tight text-[var(--text-primary)]">Jobs</h1>
        </div>
        {total > 0 && (
          <span className="rounded-full bg-[rgba(135,175,255,0.12)] px-2 py-0.5 text-[10px] font-semibold text-[var(--axon-primary)]">
            {total.toLocaleString()}
          </span>
        )}
        <div className="flex-1" />
        {hasActiveJobs && (
          <span className="flex items-center gap-1.5 animate-pulse text-[10px] text-[#38bdf8]">
            <span className="size-1.5 rounded-full bg-[#38bdf8]" />
            Live
          </span>
        )}
        <button
          type="button"
          onClick={handleRefresh}
          disabled={loading}
          className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[11px] font-medium text-[var(--text-dim)] transition-colors hover:bg-[var(--surface-float)] hover:text-[var(--axon-primary)] disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
          title="Refresh"
        >
          <RefreshCw className={`size-3.5 ${spinning ? 'animate-spin' : ''}`} />
          Refresh
        </button>
      </div>

      {/* Filter bar */}
      <div
        className="mb-3 rounded-xl border px-4 py-3"
        style={{
          background: 'var(--surface-base)',
          backdropFilter: 'blur(12px)',
          borderColor: 'var(--border-subtle)',
        }}
      >
        <div className="mb-2 flex flex-wrap items-center gap-1">
          <span className="mr-1 text-[9px] font-semibold uppercase tracking-widest text-[var(--text-dim)]">
            Type
          </span>
          {TYPE_TABS.map((t) => (
            <FilterPill
              key={t.value}
              active={typeFilter === t.value}
              onClick={() => {
                setTypeFilter(t.value)
                setOffset(0)
                setTick((n) => n + 1)
              }}
            >
              {t.label}
            </FilterPill>
          ))}
        </div>
        <div className="flex flex-wrap items-center gap-1">
          <span className="mr-1 text-[9px] font-semibold uppercase tracking-widest text-[var(--text-dim)]">
            Status
          </span>
          {STATUS_TABS.map((t) => (
            <FilterPill
              key={t.value}
              active={statusFilter === t.value}
              onClick={() => {
                setStatusFilter(t.value)
                setOffset(0)
                setTick((n) => n + 1)
              }}
            >
              {t.label}
            </FilterPill>
          ))}
        </div>
      </div>

      {/* 6. STATS BAR */}
      <StatsBar counts={counts} />

      {/* Cancel message toast */}
      {cancelMsg && (
        <div className="mb-3 rounded-lg border border-[var(--border-accent)] bg-[var(--axon-danger-bg)] px-4 py-2 text-[11px] text-[var(--axon-secondary)]">
          {cancelMsg}
        </div>
      )}

      {/* Table */}
      <div
        className="overflow-hidden rounded-xl border"
        style={{
          background: 'var(--surface-base)',
          backdropFilter: 'blur(12px)',
          borderColor: 'var(--border-subtle)',
        }}
      >
        <table className="ui-table-dense w-full">
          <thead>
            <tr>
              <th className="ui-table-head w-20 px-3 py-2.5">
                <SortableHeader column="type" label="Type" sort={sort} onSort={handleSort} />
              </th>
              <th className="ui-table-head px-3 py-2.5">
                <SortableHeader column="target" label="Target" sort={sort} onSort={handleSort} />
              </th>
              <th className="ui-table-head w-24 px-3 py-2.5">
                <SortableHeader
                  column="collection"
                  label="Collection"
                  sort={sort}
                  onSort={handleSort}
                />
              </th>
              <th className="ui-table-head w-44 px-3 py-2.5">
                <SortableHeader column="status" label="Status" sort={sort} onSort={handleSort} />
              </th>
              <th className="ui-table-head w-28 px-3 py-2.5">
                <SortableHeader column="started" label="Started" sort={sort} onSort={handleSort} />
              </th>
              <th className="ui-table-head w-20 px-3 py-2.5" />
            </tr>
          </thead>
          <tbody>
            {loading && Array.from({ length: 8 }).map((_, i) => <SkeletonRow key={i} />)}

            {!loading && error && (
              <tr>
                <td colSpan={6} className="px-4 py-10 text-center">
                  <AlertCircle className="mx-auto mb-2 size-6 text-[var(--text-dim)]" />
                  <p className="text-[12px] text-[var(--text-secondary)]">Failed to load jobs</p>
                  <p className="mt-1 text-[10px] text-[var(--text-dim)]">{error}</p>
                </td>
              </tr>
            )}

            {!loading && !error && jobs.length === 0 && (
              <tr>
                <td colSpan={6} className="px-4 py-12 text-center">
                  <Zap className="mx-auto mb-2 size-6 text-[var(--text-dim)]" />
                  <p className="text-[12px] text-[var(--text-secondary)]">No jobs found</p>
                  <p className="mt-1 text-[10px] text-[var(--text-dim)]">
                    Run a crawl, embed, or extract command to create jobs.
                  </p>
                </td>
              </tr>
            )}

            {!loading &&
              !error &&
              sortedJobs.map((job) => (
                <JobRow key={`${job.type}-${job.id}`} job={job} onCancel={handleCancel} />
              ))}

            {loadingMore &&
              Array.from({ length: 4 }).map((_, i) => <SkeletonRow key={`more-${i}`} />)}
          </tbody>
        </table>
      </div>

      {/* Load more */}
      {!loading && !error && hasMore && (
        <div className="mt-4 flex justify-center">
          <button
            type="button"
            onClick={handleLoadMore}
            disabled={loadingMore}
            className="flex items-center gap-2 rounded-lg border border-[var(--border-subtle)] bg-[var(--surface-float)] px-5 py-2 text-[11px] font-medium text-[var(--text-secondary)] transition-all hover:border-[var(--border-standard)] hover:text-[var(--axon-primary)] disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
          >
            {loadingMore ? <Loader2 className="size-3 animate-spin" /> : null}
            Load More
          </button>
        </div>
      )}

      <div className="h-12" />
    </div>
  )
}
