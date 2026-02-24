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
      className="mb-3 w-full rounded-md border border-[rgba(175,215,255,0.1)] px-3 py-1.5 text-xs text-[#dce6f0] placeholder-[#5f6b7a] outline-none transition-colors focus:border-[rgba(135,175,255,0.3)]"
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
      className={`cursor-pointer select-none border-b border-[rgba(175,215,255,0.15)] pb-2 text-[10px] uppercase tracking-wider text-[#8787af] transition-colors hover:text-[#afd7ff] ${align === 'right' ? 'text-right' : 'text-left'}`}
      onClick={() => onSort(sortKey)}
    >
      {label}
      {active && (
        <span className="ml-1 text-[#87afff]">{currentDir === 'asc' ? '\u25B2' : '\u25BC'}</span>
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
      className="text-[#87afff] transition-colors hover:text-[#afd7ff] hover:underline"
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
  const colors: Record<string, string> = {
    completed: 'bg-[#87d787]/20 text-[#87d787]',
    running: 'bg-[#87afff]/20 text-[#87afff]',
    pending: 'bg-[#ffaf87]/20 text-[#ffaf87]',
    failed: 'bg-[#ff87af]/20 text-[#ff87af]',
    canceled: 'bg-[#8787af]/20 text-[#8787af]',
  }
  const cls = colors[status] ?? 'bg-[#8787af]/20 text-[#8787af]'
  return (
    <span className={`inline-block rounded-full px-2 py-0.5 text-[10px] font-medium ${cls}`}>
      {status}
    </span>
  )
}

// ---------------------------------------------------------------------------
// Number formatter
// ---------------------------------------------------------------------------

function fmtNum(n: number): string {
  return n.toLocaleString()
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
      <div className="text-[11px] text-[#8787af] mb-2">{fmtNum(rows.length)} entries</div>
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
                className="border-b border-[rgba(175,215,255,0.05)] hover:bg-[rgba(175,215,255,0.03)]"
              >
                <td className="max-w-[400px] truncate py-1.5 pr-4">
                  <UrlCell url={row.key} />
                </td>
                <td className="py-1.5 text-right tabular-nums text-[#afd7ff]">
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
      <div className="text-[11px] text-[#8787af] mb-2">{fmtNum(rows.length)} domains</div>
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
                className="border-b border-[rgba(175,215,255,0.05)] hover:bg-[rgba(175,215,255,0.03)]"
              >
                <td className="max-w-[300px] truncate py-1.5 pr-4 text-[#dce6f0]">{row.domain}</td>
                <td className="py-1.5 text-right tabular-nums text-[#afd7ff]">
                  {fmtNum(row.urlCount)}
                </td>
                {hasTuple && (
                  <td className="py-1.5 text-right tabular-nums text-[#87d787]">
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
      <div className="mb-3 flex flex-wrap gap-4 text-[11px] text-[#8787af]">
        <span>
          Mapped: <span className="text-[#afd7ff]">{fmtNum(data.mapped_urls)}</span>
        </span>
        <span>
          Sitemap: <span className="text-[#afd7ff]">{fmtNum(data.sitemap_urls)}</span>
        </span>
        <span>
          Seen: <span className="text-[#afd7ff]">{fmtNum(data.pages_seen)}</span>
        </span>
        {data.thin_pages > 0 && (
          <span>
            Thin: <span className="text-[#ffaf87]">{fmtNum(data.thin_pages)}</span>
          </span>
        )}
        <span>
          Time: <span className="text-[#afd7ff]">{(data.elapsed_ms / 1000).toFixed(1)}s</span>
        </span>
      </div>
      <FilterInput value={filter} onChange={setFilter} />
      <div className="text-[11px] text-[#8787af] mb-2">{fmtNum(filtered.length)} URLs</div>
      <div className="max-h-[50vh] overflow-auto">
        <table className="w-full border-collapse font-mono text-[12px]">
          <thead className="sticky top-0" style={{ background: 'rgba(3, 7, 18, 0.95)' }}>
            <tr>
              <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[#8787af] w-12">
                #
              </th>
              <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[#8787af]">
                URL
              </th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((url, i) => (
              <tr
                key={url}
                className="border-b border-[rgba(175,215,255,0.05)] hover:bg-[rgba(175,215,255,0.03)]"
              >
                <td className="py-1 text-[#5f6b7a] tabular-nums">{i + 1}</td>
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
    return <div className="text-sm text-[#8787af]">No active jobs</div>
  }

  return (
    <div className="space-y-4">
      {queues.map((queue) => (
        <div key={queue.label}>
          <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-[#5f87af]">
            {queue.label} ({queue.jobs.length})
          </div>
          <div className="overflow-auto">
            <table className="w-full border-collapse font-mono text-[12px]">
              <thead>
                <tr>
                  <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[#8787af]">
                    ID
                  </th>
                  <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[#8787af]">
                    URL
                  </th>
                  <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[#8787af]">
                    Status
                  </th>
                </tr>
              </thead>
              <tbody>
                {queue.jobs.map((job) => (
                  <tr
                    key={job.id}
                    className="border-b border-[rgba(175,215,255,0.05)] hover:bg-[rgba(175,215,255,0.03)]"
                  >
                    <td className="py-1.5 text-[#8787af]">{job.id.slice(0, 8)}</td>
                    <td className="py-1.5 max-w-[300px] truncate">
                      {job.url ? (
                        <UrlCell url={job.url} />
                      ) : (
                        <span className="text-[#5f6b7a]">--</span>
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
      <div className="mb-3 flex flex-wrap gap-4 text-[11px] text-[#8787af]">
        <span>
          Collection: <span className="text-[#afd7ff]">{data.collection}</span>
        </span>
        <span>
          Indexed: <span className="text-[#afd7ff]">{fmtNum(data.indexed_urls_count)}</span>
        </span>
        <span>
          Suggestions: <span className="text-[#87d787]">{data.suggestions.length}</span>
        </span>
        {data.rejected_existing.length > 0 && (
          <span>
            Rejected: <span className="text-[#ffaf87]">{data.rejected_existing.length}</span>
          </span>
        )}
      </div>
      <FilterInput value={filter} onChange={setFilter} />
      <div className="max-h-[50vh] overflow-auto">
        <table className="w-full border-collapse font-mono text-[12px]">
          <thead className="sticky top-0" style={{ background: 'rgba(3, 7, 18, 0.95)' }}>
            <tr>
              <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[#8787af]">
                URL
              </th>
              <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-left text-[10px] uppercase tracking-wider text-[#8787af]">
                Reason
              </th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((s) => (
              <tr
                key={s.url}
                className="border-b border-[rgba(175,215,255,0.05)] hover:bg-[rgba(175,215,255,0.03)]"
              >
                <td className="py-1.5 max-w-[350px] truncate pr-4">
                  <UrlCell url={s.url} />
                </td>
                <td className="py-1.5 text-[#8787af]">{s.reason}</td>
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
      <div className="mb-3 flex flex-wrap gap-4 text-[11px] text-[#8787af]">
        <span>
          URL: <UrlCell url={data.url} />
        </span>
        <span>
          Chunks: <span className="text-[#afd7ff]">{fmtNum(data.chunks)}</span>
        </span>
      </div>
      <pre
        className="max-h-[55vh] overflow-auto whitespace-pre-wrap rounded-lg border border-[rgba(175,215,255,0.08)] p-3 font-mono text-[12px] leading-relaxed text-[#dce6f0]"
        style={{ background: 'rgba(10, 18, 35, 0.4)' }}
      >
        {data.content}
      </pre>
    </div>
  )
}
