'use client'

import { useMemo, useState } from 'react'
import type {
  DomainsResult,
  MapResult,
  NormalizedResult,
  RetrieveResult,
  SourcesResult,
  StatusResult,
  SuggestResult,
} from '@/lib/result-types'
import { fmtNum } from './shared'

type SortDir = 'asc' | 'desc'

interface TableRendererProps {
  result: NormalizedResult
}

export function TableRenderer({ result }: TableRendererProps) {
  switch (result.type) {
    case 'sources':
      return <KeyValueTable data={result.data} keyLabel="URL" valueLabel="Chunks" />
    case 'domains':
      return <DomainsTable data={result.data} />
    case 'map':
      return <MapTable data={result.data} />
    case 'status':
      return <StatusTable data={result.data} />
    case 'suggest':
      return <SuggestTable data={result.data} />
    case 'retrieve':
      return <RetrieveView data={result.data} />
    default:
      return null
  }
}

// ---------------------------------------------------------------------------
// Filter input
// ---------------------------------------------------------------------------

function FilterInput({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <input
      type="text"
      placeholder="Filter..."
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="mb-3 w-full rounded-md border border-[rgba(255,135,175,0.1)] px-3 py-1.5 text-xs text-[var(--axon-text-secondary)] placeholder-[var(--axon-text-subtle)] outline-none transition-colors focus:border-[rgba(135,175,255,0.3)]"
      style={{ background: 'rgba(10, 18, 35, 0.6)' }}
    />
  )
}

// ---------------------------------------------------------------------------
// Sortable header
// ---------------------------------------------------------------------------

function SortHeader({
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
      className={`cursor-pointer select-none border-b border-[rgba(255,135,175,0.15)] pb-2 text-[10px] uppercase tracking-wider text-[var(--axon-text-muted)] transition-colors hover:text-[var(--axon-accent-blue)] ${align === 'right' ? 'text-right' : 'text-left'}`}
      onClick={() => onSort(sortKey)}
    >
      {label}
      {active && (
        <span className="ml-1 text-[var(--axon-accent-blue-strong)]">
          {currentDir === 'asc' ? '\u25B2' : '\u25BC'}
        </span>
      )}
    </th>
  )
}

// ---------------------------------------------------------------------------
// URL cell
// ---------------------------------------------------------------------------

function UrlCell({ url }: { url: string }) {
  const isAbsolute = url.startsWith('http://') || url.startsWith('https://')
  return isAbsolute ? (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      className="text-[var(--axon-accent-blue-strong)] transition-colors hover:text-[var(--axon-accent-blue)] hover:underline"
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

function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, { color: string; background: string }> = {
    completed: { color: 'var(--axon-success)', background: 'var(--axon-success-bg)' },
    running: { color: 'var(--axon-accent-blue-strong)', background: 'rgba(135,175,255,0.14)' },
    pending: { color: 'var(--axon-warning)', background: 'var(--axon-warning-bg)' },
    failed: { color: 'var(--axon-accent-pink)', background: 'rgba(175,215,255,0.14)' },
    canceled: { color: 'var(--axon-text-muted)', background: 'rgba(147,170,202,0.14)' },
  }
  const style = colors[status] ?? {
    color: 'var(--axon-text-muted)',
    background: 'rgba(147,170,202,0.14)',
  }
  return (
    <span className="inline-block rounded-full px-2 py-0.5 text-[10px] font-medium" style={style}>
      {status}
    </span>
  )
}

// ---------------------------------------------------------------------------
// Key-value table (sources)
// ---------------------------------------------------------------------------

function KeyValueTable({
  data,
  keyLabel,
  valueLabel,
}: {
  data: SourcesResult
  keyLabel: string
  valueLabel: string
}) {
  const [filter, setFilter] = useState('')
  const [sortKey, setSortKey] = useState<string>('value')
  const [sortDir, setSortDir] = useState<SortDir>('desc')

  const rows = useMemo(() => {
    const entries = Object.entries(data)
      .filter(([k]) => k.toLowerCase().includes(filter.toLowerCase()))
      .map(([k, v]) => ({ key: k, value: v }))

    entries.sort((a, b) => {
      const cmp = sortKey === 'key' ? a.key.localeCompare(b.key) : a.value - b.value
      return sortDir === 'asc' ? cmp : -cmp
    })
    return entries
  }, [data, filter, sortKey, sortDir])

  const toggleSort = (key: string) => {
    if (sortKey === key) setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
    else {
      setSortKey(key)
      setSortDir(key === 'value' ? 'desc' : 'asc')
    }
  }

  return (
    <div>
      <FilterInput value={filter} onChange={setFilter} />
      <div className="text-[11px] text-[var(--axon-text-muted)] mb-2">
        {fmtNum(rows.length)} entries
      </div>
      <div className="max-h-[55vh] overflow-auto">
        <table className="w-full border-collapse font-mono text-[12px]">
          <thead className="sticky top-0" style={{ background: 'rgba(3, 7, 18, 0.95)' }}>
            <tr>
              <SortHeader
                label={keyLabel}
                sortKey="key"
                currentSort={sortKey}
                currentDir={sortDir}
                onSort={toggleSort}
              />
              <SortHeader
                label={valueLabel}
                sortKey="value"
                currentSort={sortKey}
                currentDir={sortDir}
                onSort={toggleSort}
                align="right"
              />
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr
                key={row.key}
                className="border-b border-[rgba(255,135,175,0.05)] hover:bg-[rgba(255,135,175,0.03)]"
              >
                <td className="max-w-[400px] truncate py-1.5 pr-4">
                  <UrlCell url={row.key} />
                </td>
                <td className="py-1.5 text-right tabular-nums text-[var(--axon-accent-blue)]">
                  {fmtNum(row.value)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Domains table
// ---------------------------------------------------------------------------

function DomainsTable({ data }: { data: DomainsResult }) {
  const [filter, setFilter] = useState('')
  const [sortKey, setSortKey] = useState<string>('count')
  const [sortDir, setSortDir] = useState<SortDir>('desc')

  const hasTuple = useMemo(() => Object.values(data).some((v) => Array.isArray(v)), [data])

  const rows = useMemo(() => {
    const entries = Object.entries(data)
      .filter(([k]) => k.toLowerCase().includes(filter.toLowerCase()))
      .map(([domain, val]) => {
        const urlCount = Array.isArray(val) ? val[0] : val
        const vecCount = Array.isArray(val) ? val[1] : val
        return { domain, urlCount, vecCount }
      })

    entries.sort((a, b) => {
      let cmp: number
      if (sortKey === 'domain') cmp = a.domain.localeCompare(b.domain)
      else if (sortKey === 'vec') cmp = a.vecCount - b.vecCount
      else cmp = a.urlCount - b.urlCount
      return sortDir === 'asc' ? cmp : -cmp
    })
    return entries
  }, [data, filter, sortKey, sortDir])

  const toggleSort = (key: string) => {
    if (sortKey === key) setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
    else {
      setSortKey(key)
      setSortDir(key === 'domain' ? 'asc' : 'desc')
    }
  }

  return (
    <div>
      <FilterInput value={filter} onChange={setFilter} />
      <div className="text-[11px] text-[var(--axon-text-muted)] mb-2">
        {fmtNum(rows.length)} domains
      </div>
      <div className="max-h-[55vh] overflow-auto">
        <table className="w-full border-collapse font-mono text-[12px]">
          <thead className="sticky top-0" style={{ background: 'rgba(3, 7, 18, 0.95)' }}>
            <tr>
              <SortHeader
                label="Domain"
                sortKey="domain"
                currentSort={sortKey}
                currentDir={sortDir}
                onSort={toggleSort}
              />
              <SortHeader
                label="URLs"
                sortKey="count"
                currentSort={sortKey}
                currentDir={sortDir}
                onSort={toggleSort}
                align="right"
              />
              {hasTuple && (
                <SortHeader
                  label="Vectors"
                  sortKey="vec"
                  currentSort={sortKey}
                  currentDir={sortDir}
                  onSort={toggleSort}
                  align="right"
                />
              )}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr
                key={row.domain}
                className="border-b border-[rgba(255,135,175,0.05)] hover:bg-[rgba(255,135,175,0.03)]"
              >
                <td className="max-w-[300px] truncate py-1.5 pr-4 text-[var(--axon-text-secondary)]">
                  {row.domain}
                </td>
                <td className="py-1.5 text-right tabular-nums text-[var(--axon-accent-blue)]">
                  {fmtNum(row.urlCount)}
                </td>
                {hasTuple && (
                  <td className="py-1.5 text-right tabular-nums text-[var(--axon-success)]">
                    {fmtNum(row.vecCount)}
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Map table (URL list)
// ---------------------------------------------------------------------------

function MapTable({ data }: { data: MapResult }) {
  const [filter, setFilter] = useState('')

  const filtered = useMemo(
    () => data.urls.filter((u) => u.toLowerCase().includes(filter.toLowerCase())),
    [data.urls, filter],
  )

  return (
    <div>
      <div className="mb-3 flex flex-wrap gap-4 text-[11px] text-[var(--axon-text-muted)]">
        <span>
          Mapped: <span className="text-[var(--axon-accent-blue)]">{fmtNum(data.mapped_urls)}</span>
        </span>
        <span>
          Sitemap:{' '}
          <span className="text-[var(--axon-accent-blue)]">{fmtNum(data.sitemap_urls)}</span>
        </span>
        <span>
          Seen: <span className="text-[var(--axon-accent-blue)]">{fmtNum(data.pages_seen)}</span>
        </span>
        {data.thin_pages > 0 && (
          <span>
            Thin: <span className="text-[var(--axon-warning)]">{fmtNum(data.thin_pages)}</span>
          </span>
        )}
        <span>
          Time:{' '}
          <span className="text-[var(--axon-accent-blue)]">
            {(data.elapsed_ms / 1000).toFixed(1)}s
          </span>
        </span>
      </div>
      <FilterInput value={filter} onChange={setFilter} />
      <div className="text-[11px] text-[var(--axon-text-muted)] mb-2">
        {fmtNum(filtered.length)} URLs
      </div>
      <div className="max-h-[50vh] overflow-auto">
        <table className="w-full border-collapse font-mono text-[12px]">
          <thead className="sticky top-0" style={{ background: 'rgba(3, 7, 18, 0.95)' }}>
            <tr>
              <th className="border-b border-[rgba(255,135,175,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[var(--axon-text-muted)] w-12">
                #
              </th>
              <th className="border-b border-[rgba(255,135,175,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[var(--axon-text-muted)]">
                URL
              </th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((url, i) => (
              <tr
                key={url}
                className="border-b border-[rgba(255,135,175,0.05)] hover:bg-[rgba(255,135,175,0.03)]"
              >
                <td className="py-1 text-[var(--axon-text-subtle)] tabular-nums">{i + 1}</td>
                <td className="py-1 truncate max-w-[600px]">
                  <UrlCell url={url} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Status table (job queues)
// ---------------------------------------------------------------------------

function StatusTable({ data }: { data: StatusResult }) {
  const queues = [
    { label: 'Crawl Jobs', jobs: data.local_crawl_jobs ?? [] },
    { label: 'Extract Jobs', jobs: data.local_extract_jobs ?? [] },
    { label: 'Embed Jobs', jobs: data.local_embed_jobs ?? [] },
    { label: 'Ingest Jobs', jobs: data.local_ingest_jobs ?? [] },
  ].filter((q) => q.jobs.length > 0)

  if (queues.length === 0) {
    return <div className="text-sm text-[var(--axon-text-muted)]">No active jobs</div>
  }

  return (
    <div className="space-y-4">
      {queues.map((queue) => (
        <div key={queue.label}>
          <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-[var(--axon-text-dim)]">
            {queue.label} ({queue.jobs.length})
          </div>
          <div className="overflow-auto">
            <table className="w-full border-collapse font-mono text-[12px]">
              <thead>
                <tr>
                  <th className="border-b border-[rgba(255,135,175,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[var(--axon-text-muted)]">
                    ID
                  </th>
                  <th className="border-b border-[rgba(255,135,175,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[var(--axon-text-muted)]">
                    URL
                  </th>
                  <th className="border-b border-[rgba(255,135,175,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[var(--axon-text-muted)]">
                    Status
                  </th>
                </tr>
              </thead>
              <tbody>
                {queue.jobs.map((job) => (
                  <tr
                    key={job.id}
                    className="border-b border-[rgba(255,135,175,0.05)] hover:bg-[rgba(255,135,175,0.03)]"
                  >
                    <td className="py-1.5 text-[var(--axon-text-muted)]">{job.id.slice(0, 8)}</td>
                    <td className="py-1.5 max-w-[300px] truncate">
                      {job.url ? (
                        <UrlCell url={job.url} />
                      ) : (
                        <span className="text-[var(--axon-text-subtle)]">--</span>
                      )}
                    </td>
                    <td className="py-1.5">
                      <StatusBadge status={job.status} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      ))}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Suggest table
// ---------------------------------------------------------------------------

function SuggestTable({ data }: { data: SuggestResult }) {
  const [filter, setFilter] = useState('')

  const filtered = useMemo(
    () =>
      data.suggestions.filter(
        (s) =>
          s.url.toLowerCase().includes(filter.toLowerCase()) ||
          s.reason.toLowerCase().includes(filter.toLowerCase()),
      ),
    [data.suggestions, filter],
  )

  return (
    <div>
      <div className="mb-3 flex flex-wrap gap-4 text-[11px] text-[var(--axon-text-muted)]">
        <span>
          Collection: <span className="text-[var(--axon-accent-blue)]">{data.collection}</span>
        </span>
        <span>
          Indexed:{' '}
          <span className="text-[var(--axon-accent-blue)]">{fmtNum(data.indexed_urls_count)}</span>
        </span>
        <span>
          Suggestions: <span className="text-[var(--axon-success)]">{data.suggestions.length}</span>
        </span>
        {data.rejected_existing.length > 0 && (
          <span>
            Rejected:{' '}
            <span className="text-[var(--axon-warning)]">{data.rejected_existing.length}</span>
          </span>
        )}
      </div>
      <FilterInput value={filter} onChange={setFilter} />
      <div className="max-h-[50vh] overflow-auto">
        <table className="w-full border-collapse font-mono text-[12px]">
          <thead className="sticky top-0" style={{ background: 'rgba(3, 7, 18, 0.95)' }}>
            <tr>
              <th className="border-b border-[rgba(255,135,175,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[var(--axon-text-muted)]">
                URL
              </th>
              <th className="border-b border-[rgba(255,135,175,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[var(--axon-text-muted)]">
                Reason
              </th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((s) => (
              <tr
                key={s.url}
                className="border-b border-[rgba(255,135,175,0.05)] hover:bg-[rgba(255,135,175,0.03)]"
              >
                <td className="py-1.5 max-w-[350px] truncate pr-4">
                  <UrlCell url={s.url} />
                </td>
                <td className="py-1.5 text-[var(--axon-text-muted)]">{s.reason}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Retrieve view (URL + chunks + content)
// ---------------------------------------------------------------------------

function RetrieveView({ data }: { data: RetrieveResult }) {
  return (
    <div>
      <div className="mb-3 flex flex-wrap gap-4 text-[11px] text-[var(--axon-text-muted)]">
        <span>
          URL: <UrlCell url={data.url} />
        </span>
        <span>
          Chunks: <span className="text-[var(--axon-accent-blue)]">{fmtNum(data.chunks)}</span>
        </span>
      </div>
      <pre
        className="max-h-[55vh] overflow-auto whitespace-pre-wrap rounded-lg border border-[rgba(255,135,175,0.08)] p-3 font-mono text-[12px] leading-relaxed text-[var(--axon-text-secondary)]"
        style={{ background: 'rgba(10, 18, 35, 0.4)' }}
      >
        {data.content}
      </pre>
    </div>
  )
}
