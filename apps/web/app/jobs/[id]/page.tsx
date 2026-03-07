'use client'

import {
  AlertCircle,
  ArrowLeft,
  CheckCircle,
  Clock,
  Database,
  ExternalLink,
  FileText,
  Globe,
  Layers,
  RefreshCw,
  Settings,
  XCircle,
} from 'lucide-react'
import Link from 'next/link'
import { use, useCallback, useEffect, useState } from 'react'
import type { JobDetail } from '@/app/api/jobs/[id]/route'
import { apiFetch } from '@/lib/api-fetch'

// ── helpers ───────────────────────────────────────────────────────────────────

function fmtDate(iso: string | null): string {
  if (!iso) return '—'
  return new Date(iso).toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

function fmtDuration(
  ms: number | null,
  startedAt: string | null,
  finishedAt: string | null,
): string {
  if (ms != null) {
    if (ms < 1000) return `${ms}ms`
    if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
    return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`
  }
  if (startedAt && finishedAt) {
    const diff = new Date(finishedAt).getTime() - new Date(startedAt).getTime()
    return fmtDuration(diff, null, null)
  }
  if (startedAt) {
    const diff = Date.now() - new Date(startedAt).getTime()
    return `${Math.floor(diff / 60000)}m ${Math.floor((diff % 60000) / 1000)}s (running)`
  }
  return '—'
}

export function shouldRefetchArtifactsOnTerminalTransition(
  previousStatus: JobDetail['status'] | null | undefined,
  nextStatus: JobDetail['status'],
): boolean {
  return previousStatus === 'running' && nextStatus !== 'running'
}

export function buildJobDetailRequestPath(id: string, includeArtifacts: boolean): string {
  const artifacts = includeArtifacts ? '1' : '0'
  return `/api/jobs/${id}?includeArtifacts=${artifacts}`
}

type FlatEntry = { key: string; value: string }

export function flattenJsonEntries(value: unknown, prefix = ''): FlatEntry[] {
  if (value === null || value === undefined) return []
  if (Array.isArray(value)) {
    return [{ key: prefix, value: JSON.stringify(value) }]
  }
  if (typeof value !== 'object') {
    return [{ key: prefix, value: String(value) }]
  }

  const obj = value as Record<string, unknown>
  const entries: FlatEntry[] = []
  for (const [k, v] of Object.entries(obj)) {
    const nextKey = prefix ? `${prefix}.${k}` : k
    if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
      entries.push(...flattenJsonEntries(v, nextKey))
    } else if (Array.isArray(v)) {
      entries.push({ key: nextKey, value: JSON.stringify(v) })
    } else if (v === null || v === undefined) {
      entries.push({ key: nextKey, value: 'null' })
    } else {
      entries.push({ key: nextKey, value: String(v) })
    }
  }
  entries.sort((a, b) => a.key.localeCompare(b.key))
  return entries
}

export function getRefreshSummaryRows(
  job: JobDetail,
): Array<{ label: string; value: string | number | null }> {
  return [
    { label: 'Checked', value: job.checked },
    { label: 'Changed', value: job.changed },
    { label: 'Unchanged', value: job.unchanged },
    { label: 'Not Modified', value: job.notModified },
    { label: 'Failed', value: job.failedCount },
    { label: 'Total', value: job.total },
    { label: 'Manifest Path', value: job.manifestPath },
  ]
}

// ── sub-components ────────────────────────────────────────────────────────────

const STATUS_CONFIG = {
  pending: {
    label: 'Pending',
    color: 'text-[var(--text-dim)]',
    bg: 'bg-[rgba(135,175,255,0.08)]',
    icon: Clock,
  },
  running: {
    label: 'Running',
    color: 'text-[var(--axon-primary)]',
    bg: 'bg-[rgba(135,175,255,0.12)]',
    icon: RefreshCw,
  },
  completed: {
    label: 'Completed',
    color: 'text-[var(--axon-success,#87d7af)]',
    bg: 'bg-[rgba(135,215,175,0.12)]',
    icon: CheckCircle,
  },
  failed: {
    label: 'Failed',
    color: 'text-[var(--axon-secondary)]',
    bg: 'bg-[rgba(255,135,175,0.12)]',
    icon: XCircle,
  },
  canceled: {
    label: 'Canceled',
    color: 'text-[var(--text-muted)]',
    bg: 'bg-[rgba(135,135,175,0.1)]',
    icon: XCircle,
  },
} as const

function StatusBadge({ status }: { status: JobDetail['status'] }) {
  const cfg = STATUS_CONFIG[status] ?? STATUS_CONFIG.pending
  const Icon = cfg.icon
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded px-2 py-0.5 text-xs font-medium ${cfg.color} ${cfg.bg}`}
    >
      <Icon className={`size-3.5 ${status === 'running' ? 'animate-spin' : ''}`} />
      {cfg.label}
    </span>
  )
}

const TYPE_COLORS: Record<string, string> = {
  crawl: 'text-[var(--axon-primary)]   bg-[rgba(135,175,255,0.1)]',
  embed: 'text-[var(--axon-secondary)] bg-[rgba(255,135,175,0.1)]',
  extract: 'text-[#d7af87]               bg-[rgba(215,175,135,0.1)]',
  ingest: 'text-[#87d7d7]               bg-[rgba(135,215,215,0.1)]',
  refresh: 'text-[#34d399]              bg-[rgba(52,211,153,0.1)]',
}

function TypeBadge({ type }: { type: string }) {
  return (
    <span
      className={`inline-flex items-center rounded px-2 py-0.5 text-xs font-semibold uppercase tracking-wider ${TYPE_COLORS[type] ?? ''}`}
    >
      {type}
    </span>
  )
}

function Stat({
  label,
  value,
  icon: Icon,
}: {
  label: string
  value: string | number | null
  icon?: React.ElementType
}) {
  return (
    <div className="flex flex-col gap-1 rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.6)] px-4 py-3">
      <div className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
        {Icon && <Icon className="size-3" />}
        {label}
      </div>
      <div className="font-mono text-lg font-bold text-[var(--text-primary)]">{value ?? '—'}</div>
    </div>
  )
}

function Section({
  title,
  icon: Icon,
  children,
}: {
  title: string
  icon: React.ElementType
  children: React.ReactNode
}) {
  return (
    <div className="rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.5)]">
      <div className="flex items-center gap-2 border-b border-[var(--border-subtle)] px-4 py-2.5">
        <Icon className="size-4 text-[var(--text-dim)]" />
        <span className="text-xs font-semibold uppercase tracking-wider text-[var(--text-dim)]">
          {title}
        </span>
      </div>
      <div className="p-4">{children}</div>
    </div>
  )
}

function KV({
  label,
  value,
  mono = false,
}: {
  label: string
  value: React.ReactNode
  mono?: boolean
}) {
  return (
    <div className="flex items-start gap-3 py-1.5 border-b border-[var(--border-subtle)] last:border-0">
      <span className="w-36 flex-shrink-0 text-[11px] text-[var(--text-dim)]">{label}</span>
      <span
        className={`min-w-0 break-all text-[11px] text-[var(--text-secondary)] ${mono ? 'font-mono' : ''}`}
      >
        {value ?? '—'}
      </span>
    </div>
  )
}

function ShowMoreList<T>({
  title,
  items,
  emptyText,
  initial = 200,
  step = 500,
  renderItem,
}: {
  title: string
  items: T[]
  emptyText: string
  initial?: number
  step?: number
  renderItem: (item: T, index: number) => React.ReactNode
}) {
  const [visible, setVisible] = useState(initial)
  const shown = items.slice(0, visible)
  const hasMore = visible < items.length

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <div className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
          {title}
        </div>
        <div className="font-mono text-[10px] text-[var(--text-muted)]">
          {items.length.toLocaleString()} total
        </div>
      </div>
      {items.length === 0 ? (
        <div className="rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.35)] px-3 py-2 text-[11px] text-[var(--text-muted)]">
          {emptyText}
        </div>
      ) : (
        <ul className="max-h-[22rem] space-y-1 overflow-auto rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.35)] p-2">
          {shown.map((item, idx) => (
            <li key={idx}>{renderItem(item, idx)}</li>
          ))}
        </ul>
      )}
      {hasMore && (
        <button
          type="button"
          onClick={() => setVisible((prev) => prev + step)}
          className="rounded border border-[var(--border-subtle)] px-2 py-1 text-[11px] text-[var(--axon-primary)] hover:bg-[rgba(135,175,255,0.1)]"
        >
          Show {Math.min(step, items.length - visible).toLocaleString()} More
        </button>
      )}
    </div>
  )
}

// ── main page ─────────────────────────────────────────────────────────────────

export default function JobDetailPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params)
  const [job, setJob] = useState<JobDetail | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  const fetchJob = useCallback(
    async (includeArtifacts = true) => {
      try {
        const res = await apiFetch(buildJobDetailRequestPath(id, includeArtifacts))
        if (!res.ok) {
          const body = (await res.json()) as { error?: string }
          setError(body.error ?? `HTTP ${res.status}`)
          return
        }
        const data = (await res.json()) as JobDetail
        let needsArtifactRefetch = false
        setJob((previous) => {
          if (
            previous &&
            !includeArtifacts &&
            shouldRefetchArtifactsOnTerminalTransition(previous.status, data.status)
          ) {
            needsArtifactRefetch = true
          }
          if (!previous) return data
          return {
            ...data,
            observedUrls: data.observedUrls ?? previous.observedUrls,
            markdownFiles: data.markdownFiles ?? previous.markdownFiles,
            thinUrls: data.thinUrls ?? previous.thinUrls,
            wafBlockedUrls: data.wafBlockedUrls ?? previous.wafBlockedUrls,
          }
        })
        if (needsArtifactRefetch) {
          void fetchJob(true)
        }
        setError(null)
      } catch {
        setError('Failed to fetch job')
      } finally {
        setLoading(false)
      }
    },
    [id],
  )

  useEffect(() => {
    void fetchJob(true)
  }, [fetchJob])

  // Poll every 3s while running
  useEffect(() => {
    if (!job || job.status !== 'running') return
    const interval = setInterval(() => void fetchJob(false), 3000)
    return () => clearInterval(interval)
  }, [job, fetchJob])

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center text-[var(--text-dim)]">
        <RefreshCw className="size-5 animate-spin mr-2" />
        Loading job…
      </div>
    )
  }

  if (error || !job) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 text-[var(--text-muted)]">
        <AlertCircle className="size-10 text-[var(--axon-secondary)]" />
        <p className="text-sm">{error ?? 'Job not found'}</p>
        <Link
          href="/jobs"
          className="flex items-center gap-1.5 rounded px-3 py-1.5 text-xs text-[var(--axon-primary)] hover:bg-[rgba(135,175,255,0.1)] transition-colors"
        >
          <ArrowLeft className="size-3.5" />
          Back to Jobs
        </Link>
      </div>
    )
  }

  const duration = fmtDuration(job.elapsedMs, job.startedAt, job.finishedAt)
  const isUrl = job.type === 'crawl' || (job.type === 'embed' && job.target.startsWith('http'))
  const resultJsonFlat = flattenJsonEntries(job.resultJson)
  const configJsonFlat = flattenJsonEntries(job.configJson)

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-3 border-b border-[var(--border-subtle)] px-5 py-3 flex-shrink-0">
        <Link
          href="/jobs"
          className="flex items-center gap-1.5 rounded px-2 py-1 text-xs text-[var(--text-muted)] hover:bg-[rgba(135,175,255,0.08)] hover:text-[var(--text-secondary)] transition-colors"
        >
          <ArrowLeft className="size-3.5" />
          Jobs
        </Link>
        <span className="text-[var(--text-dim)]">/</span>
        <TypeBadge type={job.type} />
        <StatusBadge status={job.status} />
        {job.status === 'running' && (
          <span className="ml-auto text-[10px] text-[var(--text-dim)] animate-pulse">
            Auto-refreshing…
          </span>
        )}
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto px-5 py-5 space-y-5">
        {/* Target URL */}
        <div className="rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.6)] px-4 py-3">
          <div className="flex items-start gap-2">
            <Globe className="size-4 text-[var(--text-dim)] mt-0.5 flex-shrink-0" />
            <div className="min-w-0 flex-1">
              <div className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-dim)] mb-1">
                Target
              </div>
              {isUrl ? (
                <a
                  href={job.target}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="break-all font-mono text-sm text-[var(--axon-primary)] hover:underline flex items-center gap-1.5"
                >
                  {job.target}
                  <ExternalLink className="size-3 flex-shrink-0" />
                </a>
              ) : (
                <span className="break-all font-mono text-sm text-[var(--text-secondary)]">
                  {job.target}
                </span>
              )}
            </div>
          </div>
        </div>

        {/* ID */}
        <div className="flex items-center gap-2 rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.4)] px-4 py-2.5">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-dim)] w-16 flex-shrink-0">
            Job ID
          </span>
          <code className="break-all font-mono text-[11px] text-[var(--text-secondary)]">
            {job.id}
          </code>
        </div>

        {/* Stats grid (crawl) */}
        {job.type === 'crawl' && (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Stat label="Pages Crawled" value={job.pagesCrawled} icon={Globe} />
            <Stat label="Pages Discovered" value={job.pagesDiscovered} icon={Globe} />
            <Stat label="Markdown Created" value={job.mdCreated} icon={FileText} />
            <Stat label="Thin Skipped" value={job.thinMd} icon={FileText} />
            <Stat label="Filtered URLs" value={job.filteredUrls} icon={Globe} />
            <Stat label="Error Pages" value={job.errorPages} icon={AlertCircle} />
            <Stat label="WAF Blocked" value={job.wafBlockedPages} icon={AlertCircle} />
            <Stat
              label="Success"
              value={job.success == null ? 'running' : job.success ? 'yes' : 'no'}
              icon={job.success == null ? Clock : job.success ? CheckCircle : XCircle}
            />
          </div>
        )}

        {/* Stats grid (embed) */}
        {job.type === 'embed' && (
          <div className="grid grid-cols-2 gap-3">
            <Stat label="Docs Embedded" value={job.docsEmbedded} icon={FileText} />
            <Stat label="Chunks Embedded" value={job.chunksEmbedded} icon={Database} />
          </div>
        )}

        {/* Extract URLs */}
        {job.type === 'extract' && job.urls && job.urls.length > 0 && (
          <Section title="URLs" icon={Globe}>
            <ul className="space-y-1">
              {job.urls.map((u) => {
                const isSafeUrl = /^https?:\/\//i.test(u)
                return (
                  <li key={u}>
                    {isSafeUrl ? (
                      <a
                        href={u}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="font-mono text-[11px] text-[var(--axon-primary)] hover:underline flex items-center gap-1"
                      >
                        {u} <ExternalLink className="size-3 flex-shrink-0" />
                      </a>
                    ) : (
                      <span className="font-mono text-[11px] text-[var(--axon-primary)]">{u}</span>
                    )}
                  </li>
                )
              })}
            </ul>
          </Section>
        )}

        {job.type === 'refresh' && (
          <Section title="Refresh Summary" icon={RefreshCw}>
            <div className="space-y-0">
              {getRefreshSummaryRows(job).map((row) => (
                <KV
                  key={row.label}
                  label={row.label}
                  value={row.value}
                  mono={row.label === 'Manifest Path'}
                />
              ))}
            </div>
            {job.urls && job.urls.length > 0 && (
              <div className="mt-3">
                <ShowMoreList
                  title="Refresh URLs"
                  items={job.urls}
                  emptyText="No refresh URLs recorded."
                  renderItem={(url) => (
                    <a
                      href={url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center gap-1 break-all font-mono text-[11px] text-[var(--axon-primary)] hover:underline"
                    >
                      {url}
                      <ExternalLink className="size-3 flex-shrink-0" />
                    </a>
                  )}
                />
              </div>
            )}
          </Section>
        )}

        {/* Error */}
        {job.errorText && (
          <div className="rounded border border-[rgba(255,135,175,0.3)] bg-[rgba(255,135,175,0.06)] px-4 py-3">
            <div className="flex items-center gap-2 mb-2">
              <XCircle className="size-4 text-[var(--axon-secondary)]" />
              <span className="text-xs font-semibold text-[var(--axon-secondary)]">Error</span>
            </div>
            <pre className="whitespace-pre-wrap font-mono text-[11px] text-[var(--text-secondary)]">
              {job.errorText}
            </pre>
          </div>
        )}

        {/* Timing */}
        <Section title="Timing" icon={Clock}>
          <div className="space-y-0">
            <KV label="Created" value={fmtDate(job.createdAt)} />
            <KV label="Started" value={fmtDate(job.startedAt)} />
            <KV label="Finished" value={fmtDate(job.finishedAt)} />
            <KV label="Duration" value={duration} mono />
          </div>
        </Section>

        {/* Config */}
        <Section title="Configuration" icon={Settings}>
          <div className="space-y-0">
            {job.collection && <KV label="Collection" value={job.collection} mono />}
            {job.renderMode && <KV label="Render Mode" value={job.renderMode} mono />}
            {job.maxDepth != null && <KV label="Max Depth" value={job.maxDepth} />}
            {job.maxPages != null && (
              <KV label="Max Pages" value={job.maxPages === 0 ? 'unlimited' : job.maxPages} />
            )}
            {job.embed != null && <KV label="Auto-embed" value={job.embed ? 'yes' : 'no'} />}
            {job.cacheHit != null && <KV label="Cache Hit" value={job.cacheHit ? 'yes' : 'no'} />}
            {job.outputDir && <KV label="Output Dir" value={job.outputDir} mono />}
            {job.staleUrlsDeleted != null && (
              <KV label="Stale URLs Deleted" value={job.staleUrlsDeleted} />
            )}
            {typeof job.resultJson?.manifest_path === 'string' && (
              <KV label="Manifest Path" value={job.resultJson.manifest_path} mono />
            )}
            {typeof job.resultJson?.audit_report_path === 'string' && (
              <KV label="Audit Report Path" value={job.resultJson.audit_report_path} mono />
            )}
          </div>
        </Section>

        {job.type === 'crawl' && (
          <Section title="Crawl Artifacts" icon={FileText}>
            <div className="space-y-4">
              <ShowMoreList
                title="Observed URLs"
                items={job.observedUrls ?? []}
                emptyText="No URL artifacts found."
                renderItem={(url) => (
                  <a
                    href={url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="flex items-center gap-1 break-all font-mono text-[11px] text-[var(--axon-primary)] hover:underline"
                  >
                    {url}
                    <ExternalLink className="size-3 flex-shrink-0" />
                  </a>
                )}
              />
              <ShowMoreList
                title="Markdown Files Created"
                items={job.markdownFiles ?? []}
                emptyText="No manifest file entries found."
                renderItem={(entry) => (
                  <div className="space-y-0.5 rounded border border-[var(--border-subtle)] bg-[rgba(0,0,0,0.2)] px-2 py-1.5">
                    <div className="break-all font-mono text-[11px] text-[var(--text-secondary)]">
                      {entry.relativePath}
                    </div>
                    <div className="break-all font-mono text-[10px] text-[var(--text-muted)]">
                      {entry.url}
                    </div>
                    <div className="text-[10px] text-[var(--text-dim)]">
                      {entry.markdownChars.toLocaleString()} chars
                    </div>
                  </div>
                )}
              />
              <ShowMoreList
                title="Thin URLs"
                items={job.thinUrls ?? []}
                emptyText="No thin URL list recorded."
                renderItem={(url) => (
                  <span className="break-all font-mono text-[11px] text-[var(--text-secondary)]">
                    {url}
                  </span>
                )}
              />
              <ShowMoreList
                title="WAF Blocked URLs"
                items={job.wafBlockedUrls ?? []}
                emptyText="No WAF-blocked URL list recorded."
                renderItem={(url) => (
                  <span className="break-all font-mono text-[11px] text-[var(--text-secondary)]">
                    {url}
                  </span>
                )}
              />
            </div>
          </Section>
        )}

        {/* Raw result JSON */}
        <Section title="Result JSON Metadata" icon={Layers}>
          <div className="space-y-0">
            {resultJsonFlat.length === 0 ? (
              <KV label="result_json" value="—" />
            ) : (
              resultJsonFlat.map((entry) => (
                <KV key={entry.key} label={entry.key} value={entry.value} mono />
              ))
            )}
          </div>
        </Section>

        <Section title="Config JSON Metadata" icon={Settings}>
          <div className="space-y-0">
            {configJsonFlat.length === 0 ? (
              <KV label="config_json" value="—" />
            ) : (
              configJsonFlat.map((entry) => (
                <KV key={entry.key} label={entry.key} value={entry.value} mono />
              ))
            )}
          </div>
        </Section>
        {job.resultJson && Object.keys(job.resultJson).length > 0 && (
          <Section title="Result Data" icon={Layers}>
            <pre className="overflow-x-auto rounded bg-[rgba(0,0,0,0.3)] p-3 font-mono text-[10px] text-[var(--text-dim)] leading-relaxed">
              {JSON.stringify(job.resultJson, null, 2)}
            </pre>
          </Section>
        )}
      </div>
    </div>
  )
}
