'use client'

import {
  AlertCircle,
  Ban,
  CheckCircle2,
  Clock,
  ExternalLink,
  Loader2,
  RefreshCw,
  Zap,
} from 'lucide-react'
import Link from 'next/link'
import { useEffect, useRef, useState } from 'react'
import type { Job, JobStatus, JobType } from '@/app/api/jobs/route'

// ── Types ──────────────────────────────────────────────────────────────────────

type TypeFilter = 'all' | JobType
type StatusFilter = 'all' | 'active' | 'pending' | 'completed' | 'failed'

interface JobsApiResponse {
  jobs: Job[]
  total: number
  hasMore: boolean
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

// ── Status badge ───────────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: JobStatus }) {
  const base = 'ui-chip-status border'
  const variants: Record<JobStatus, { cls: string; icon: React.ReactNode; label: string }> = {
    pending: {
      cls: `${base} text-[var(--text-muted)] border-[var(--border-subtle)]`,
      icon: <Clock className="size-2.5" />,
      label: 'Pending',
    },
    running: {
      cls: `${base} text-[var(--axon-primary)] border-[rgba(135,175,255,0.5)] animate-pulse`,
      icon: <Loader2 className="size-2.5 animate-spin" />,
      label: 'Running',
    },
    completed: {
      cls: `${base} text-[var(--axon-success)] border-[rgba(130,217,160,0.4)]`,
      icon: <CheckCircle2 className="size-2.5" />,
      label: 'Done',
    },
    failed: {
      cls: `${base} text-red-400 border-red-400/40`,
      icon: <AlertCircle className="size-2.5" />,
      label: 'Failed',
    },
    canceled: {
      cls: `${base} text-yellow-400 border-yellow-400/40`,
      icon: <Ban className="size-2.5" />,
      label: 'Canceled',
    },
  }
  const v = variants[status]
  return (
    <span className={v.cls}>
      {v.icon}
      {v.label}
    </span>
  )
}

// ── Type chip ─────────────────────────────────────────────────────────────────

function TypeChip({ type }: { type: JobType }) {
  const colors: Record<JobType, string> = {
    crawl: 'text-[var(--axon-primary)] bg-[rgba(135,175,255,0.12)]',
    extract: 'text-[var(--axon-secondary)] bg-[rgba(255,135,175,0.12)]',
    embed: 'text-[var(--axon-success)] bg-[rgba(130,217,160,0.12)]',
    ingest: 'text-[var(--axon-warning)] bg-[rgba(255,192,134,0.12)]',
  }
  return (
    <span
      className={`inline-flex items-center rounded px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-widest ${colors[type]}`}
    >
      {type}
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
        'min-h-[44px] sm:min-h-0',
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
        <div className={`${shimmer} h-4 w-16 rounded-full`} />
      </td>
      <td className="px-3 py-2.5">
        <div className={`${shimmer} h-3 w-20`} />
      </td>
      <td className="px-3 py-2.5">
        <div className={`${shimmer} h-5 w-5 rounded`} />
      </td>
    </tr>
  )
}

// ── Job row ───────────────────────────────────────────────────────────────────

function JobRow({ job, onCancel }: { job: Job; onCancel: (id: string, type: JobType) => void }) {
  const canCancel = job.status === 'pending' || job.status === 'running'
  const started = job.startedAt
    ? new Date(job.startedAt).toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      })
    : '—'

  return (
    <tr
      className="border-b border-[var(--border-subtle)] transition-colors duration-100 hover:bg-[rgba(135,175,255,0.03)]"
      title={job.errorText ?? undefined}
    >
      <td className="px-3 py-2.5">
        <TypeChip type={job.type} />
      </td>
      <td className="px-3 py-2.5 max-w-[300px]">
        <Link
          href={`/jobs/${job.id}`}
          className="group flex items-center gap-1.5 min-w-0"
          title={job.target}
        >
          <span className="block truncate font-mono text-[11px] text-[var(--text-secondary)] group-hover:text-[var(--axon-primary)] transition-colors">
            {job.target}
          </span>
          <ExternalLink className="size-3 flex-shrink-0 text-[var(--text-dim)] opacity-0 group-hover:opacity-100 transition-opacity" />
        </Link>
      </td>
      <td className="px-3 py-2.5">
        {job.collection ? (
          <span
            className="font-mono text-[10px] text-[var(--text-dim)] bg-[rgba(135,175,255,0.07)] rounded px-1.5 py-0.5 whitespace-nowrap"
            title={job.collection}
          >
            {job.collection}
          </span>
        ) : (
          <span className="text-[10px] text-[var(--text-dim)]">—</span>
        )}
      </td>
      <td className="px-3 py-2.5">
        <StatusBadge status={job.status} />
      </td>
      <td className="px-3 py-2.5 whitespace-nowrap font-mono text-[10px] text-[var(--text-dim)]">
        {started}
      </td>
      <td className="px-3 py-2.5">
        {canCancel && (
          <button
            type="button"
            onClick={() => onCancel(job.id, job.type)}
            className="rounded p-1 text-[var(--text-dim)] transition-colors hover:bg-[rgba(255,135,175,0.1)] hover:text-[var(--axon-secondary)] min-h-[44px] min-w-[44px] sm:min-h-0 sm:min-w-0 focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
            title="Cancel job (not yet supported)"
            aria-label="Cancel job"
          >
            <Ban className="size-3.5" />
          </button>
        )}
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
  // Tick increments whenever we want the main fetch effect to re-run
  const [tick, setTick] = useState(0)
  const pollRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const hasActiveJobs = jobs.some((j) => j.status === 'pending' || j.status === 'running')

  // Ref so handlers below can always access latest filter values without re-subscribing effects
  const filterRef = useRef({ typeFilter, statusFilter, offset })
  filterRef.current = { typeFilter, statusFilter, offset }

  // Core fetch — runs whenever tick changes (filter change, manual refresh, poll)
  // biome-ignore lint/correctness/useExhaustiveDependencies: tick is an imperative trigger; filterRef provides latest values without re-subscribing
  useEffect(() => {
    const controller = new AbortController()
    const { typeFilter: type, statusFilter: status } = filterRef.current

    async function run() {
      setLoading(true)
      setError(null)
      try {
        const params = new URLSearchParams({
          type,
          status,
          limit: String(PAGE_SIZE),
          offset: '0',
        })
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

  // Auto-poll while active jobs exist — increments tick to trigger re-fetch
  // biome-ignore lint/correctness/useExhaustiveDependencies: tick is included to reset poll timer after each fetch cycle; hasActiveJobs controls whether polling is active
  useEffect(() => {
    if (!hasActiveJobs) return
    pollRef.current = setTimeout(() => {
      setTick((t) => t + 1)
    }, POLL_INTERVAL_MS)
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

  return (
    <div className="p-6 max-w-6xl mx-auto animate-fade-in">
      {/* Header */}
      <div className="mb-6 flex items-center gap-3">
        <div className="flex items-center gap-2">
          <Zap className="size-4 text-[var(--axon-primary)]" />
          <h1 className="text-[18px] font-bold text-[var(--text-primary)] tracking-tight">Jobs</h1>
        </div>
        {total > 0 && (
          <span className="rounded-full bg-[rgba(135,175,255,0.12)] px-2 py-0.5 text-[10px] font-semibold text-[var(--axon-primary)]">
            {total.toLocaleString()}
          </span>
        )}
        <div className="flex-1" />
        {hasActiveJobs && (
          <span className="flex items-center gap-1.5 text-[10px] text-[var(--axon-primary)] animate-pulse">
            <span className="size-1.5 rounded-full bg-[var(--axon-primary)]" />
            Live
          </span>
        )}
        <button
          type="button"
          onClick={handleRefresh}
          disabled={loading}
          className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[11px] font-medium text-[var(--text-dim)] transition-colors hover:bg-[var(--surface-float)] hover:text-[var(--axon-primary)] disabled:opacity-40 min-h-[44px] sm:min-h-0 focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
          title="Refresh"
        >
          <RefreshCw className={`size-3.5 ${spinning ? 'animate-spin' : ''}`} />
          Refresh
        </button>
      </div>

      {/* Filter bar */}
      <div
        className="mb-4 rounded-xl border px-4 py-3"
        style={{
          background: 'var(--surface-base)',
          backdropFilter: 'blur(12px)',
          borderColor: 'var(--border-subtle)',
        }}
      >
        {/* Type filters */}
        <div className="mb-2 flex items-center gap-1 flex-wrap">
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
        {/* Status filters */}
        <div className="flex items-center gap-1 flex-wrap">
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

      {/* Cancel message toast */}
      {cancelMsg && (
        <div className="mb-3 rounded-lg border border-[var(--border-accent)] bg-[var(--axon-danger-bg)] px-4 py-2 text-[11px] text-[var(--axon-secondary)]">
          {cancelMsg}
        </div>
      )}

      {/* Table */}
      <div
        className="rounded-xl border overflow-hidden"
        style={{
          background: 'var(--surface-base)',
          backdropFilter: 'blur(12px)',
          borderColor: 'var(--border-subtle)',
        }}
      >
        <table className="ui-table-dense w-full">
          <thead>
            <tr>
              <th className="ui-table-head px-3 py-2.5 w-20">Type</th>
              <th className="ui-table-head px-3 py-2.5">Target</th>
              <th className="ui-table-head px-3 py-2.5 w-24">Collection</th>
              <th className="ui-table-head px-3 py-2.5 w-28">Status</th>
              <th className="ui-table-head px-3 py-2.5 w-36">Started</th>
              <th className="ui-table-head px-3 py-2.5 w-10" />
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
              jobs.map((job) => (
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
            className="flex items-center gap-2 rounded-lg border border-[var(--border-subtle)] bg-[var(--surface-float)] px-5 py-2 text-[11px] font-medium text-[var(--text-secondary)] transition-all hover:border-[var(--border-standard)] hover:text-[var(--axon-primary)] disabled:opacity-40 min-h-[44px] sm:min-h-0 focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
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
