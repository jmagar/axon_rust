'use client'

import { AlertCircle, Loader2, RefreshCw, Zap } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { Job, JobStatus, JobType, StatusCounts } from '@/app/api/jobs/route'
import { apiFetch } from '@/lib/api-fetch'
import {
  JobRow,
  SkeletonRow,
  SortableHeader,
  type SortColumn,
  type SortDir,
  StatsBar,
} from './job-cells'

// ── Types ─────────────────────────────────────────────────────────────────────

type TypeFilter = 'all' | JobType
type StatusFilter = 'all' | 'active' | 'pending' | 'completed' | 'failed'

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

// ── 7. CLIENT-SIDE SORT ───────────────────────────────────────────────────────

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
  const [counts, setCounts] = useState<StatusCounts | undefined>(undefined)
  const [sort, setSort] = useState<{ column: SortColumn; dir: SortDir }>({
    column: 'started',
    dir: 'desc',
  })
  const [tick, setTick] = useState(0)
  const pollRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const hasActiveJobs = jobs.some((j) => j.status === 'pending' || j.status === 'running')

  const filterRef = useRef({ typeFilter, statusFilter, offset })
  filterRef.current = { typeFilter, statusFilter, offset }

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
        const res = await apiFetch(`/api/jobs?${params}`, { signal: controller.signal })
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
    apiFetch(`/api/jobs?${params}`)
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
          <span className="flex animate-pulse items-center gap-1.5 text-[10px] text-[#38bdf8]">
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
            </tr>
          </thead>
          <tbody>
            {loading && Array.from({ length: 8 }).map((_, i) => <SkeletonRow key={i} />)}

            {!loading && error && (
              <tr>
                <td colSpan={5} className="px-4 py-10 text-center">
                  <AlertCircle className="mx-auto mb-2 size-6 text-[var(--text-dim)]" />
                  <p className="text-[12px] text-[var(--text-secondary)]">Failed to load jobs</p>
                  <p className="mt-1 text-[10px] text-[var(--text-dim)]">{error}</p>
                </td>
              </tr>
            )}

            {!loading && !error && jobs.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-12 text-center">
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
              sortedJobs.map((job) => <JobRow key={`${job.type}-${job.id}`} job={job} />)}

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
