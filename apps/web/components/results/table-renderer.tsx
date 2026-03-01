'use client'

import { useMemo, useRef, useState } from 'react'
import type { DomainsResult, MapResult, NormalizedResult, SourcesResult } from '@/lib/result-types'
import {
  DISPLAY_LIMIT,
  FilterInput,
  fmtNum,
  type SortDir,
  SortHeader,
  TopNToggle,
  UrlCell,
  VIRTUAL_THRESHOLD,
  VirtualTableBody,
} from './table-primitives'
import { RetrieveView, StatusTable, SuggestTable } from './table-views'

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
  const [showAll, setShowAll] = useState(false)
  const parentRef = useRef<HTMLDivElement>(null)

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

  const displayRows = rows.length > 1000 && !showAll ? rows.slice(0, DISPLAY_LIMIT) : rows
  const shouldVirtualize = displayRows.length > VIRTUAL_THRESHOLD

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
      <div className="ui-meta mb-2">{fmtNum(rows.length)} entries</div>
      <TopNToggle
        totalRows={rows.length}
        showAll={showAll}
        onToggle={() => setShowAll((v) => !v)}
      />
      <div
        ref={parentRef}
        className="max-h-[60vh] overflow-auto"
        style={shouldVirtualize ? { height: '60vh' } : undefined}
      >
        <table className="w-full table-fixed text-left">
          <thead className="sticky top-0" style={{ background: 'var(--surface-base)' }}>
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
          {shouldVirtualize ? (
            <VirtualTableBody
              rows={displayRows}
              parentRef={parentRef}
              renderRow={(row) => (
                <>
                  <td className="ui-table-cell max-w-[400px] truncate pr-4">
                    <UrlCell url={row.key} />
                  </td>
                  <td className="ui-table-cell text-right tabular-nums text-[var(--axon-primary)]">
                    {fmtNum(row.value)}
                  </td>
                </>
              )}
            />
          ) : (
            <tbody>
              {displayRows.map((row, idx) => (
                <tr
                  key={row.key}
                  className="border-b border-[var(--border-subtle)] hover:bg-[var(--surface-float)] transition-colors animate-fade-in-up"
                  style={{ animationDelay: `${idx * 20}ms` }}
                >
                  <td className="ui-table-cell max-w-[400px] truncate pr-4">
                    <UrlCell url={row.key} />
                  </td>
                  <td className="ui-table-cell text-right tabular-nums text-[var(--axon-primary)]">
                    {fmtNum(row.value)}
                  </td>
                </tr>
              ))}
            </tbody>
          )}
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
  const [showAll, setShowAll] = useState(false)
  const parentRef = useRef<HTMLDivElement>(null)

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

  const displayRows = rows.length > 1000 && !showAll ? rows.slice(0, DISPLAY_LIMIT) : rows
  const shouldVirtualize = displayRows.length > VIRTUAL_THRESHOLD

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
      <div className="ui-meta mb-2">{fmtNum(rows.length)} domains</div>
      <TopNToggle
        totalRows={rows.length}
        showAll={showAll}
        onToggle={() => setShowAll((v) => !v)}
      />
      <div
        ref={parentRef}
        className="max-h-[60vh] overflow-auto"
        style={shouldVirtualize ? { height: '60vh' } : undefined}
      >
        <table className="w-full table-fixed text-left">
          <thead className="sticky top-0" style={{ background: 'var(--surface-base)' }}>
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
          {shouldVirtualize ? (
            <VirtualTableBody
              rows={displayRows}
              parentRef={parentRef}
              renderRow={(row) => (
                <>
                  <td className="ui-table-cell max-w-[300px] truncate pr-4 text-[var(--text-secondary)]">
                    {row.domain}
                  </td>
                  <td className="ui-table-cell text-right tabular-nums text-[var(--axon-primary)]">
                    {fmtNum(row.urlCount)}
                  </td>
                  {hasTuple && (
                    <td className="ui-table-cell text-right tabular-nums text-[var(--axon-success)]">
                      {fmtNum(row.vecCount)}
                    </td>
                  )}
                </>
              )}
            />
          ) : (
            <tbody>
              {displayRows.map((row, idx) => (
                <tr
                  key={row.domain}
                  className="border-b border-[var(--border-subtle)] hover:bg-[var(--surface-float)] transition-colors animate-fade-in-up"
                  style={{ animationDelay: `${idx * 20}ms` }}
                >
                  <td className="ui-table-cell max-w-[300px] truncate pr-4 text-[var(--text-secondary)]">
                    {row.domain}
                  </td>
                  <td className="ui-table-cell text-right tabular-nums text-[var(--axon-primary)]">
                    {fmtNum(row.urlCount)}
                  </td>
                  {hasTuple && (
                    <td className="ui-table-cell text-right tabular-nums text-[var(--axon-success)]">
                      {fmtNum(row.vecCount)}
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          )}
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
  const [showAll, setShowAll] = useState(false)
  const parentRef = useRef<HTMLDivElement>(null)

  const filtered = useMemo(
    () => data.urls.filter((u) => u.toLowerCase().includes(filter.toLowerCase())),
    [data.urls, filter],
  )

  const displayRows =
    filtered.length > 1000 && !showAll ? filtered.slice(0, DISPLAY_LIMIT) : filtered
  const shouldVirtualize = displayRows.length > VIRTUAL_THRESHOLD

  return (
    <div>
      <div className="ui-meta mb-3 flex flex-wrap gap-4">
        <span>
          Mapped: <span className="text-[var(--axon-primary)]">{fmtNum(data.mapped_urls)}</span>
        </span>
        <span>
          Sitemap: <span className="text-[var(--axon-primary)]">{fmtNum(data.sitemap_urls)}</span>
        </span>
        <span>
          Seen: <span className="text-[var(--axon-primary)]">{fmtNum(data.pages_seen)}</span>
        </span>
        {data.thin_pages > 0 && (
          <span>
            Thin: <span className="text-[var(--axon-warning)]">{fmtNum(data.thin_pages)}</span>
          </span>
        )}
        <span>
          Time:{' '}
          <span className="text-[var(--axon-primary)]">{(data.elapsed_ms / 1000).toFixed(1)}s</span>
        </span>
      </div>
      <FilterInput value={filter} onChange={setFilter} />
      <div className="ui-meta mb-2">{fmtNum(filtered.length)} URLs</div>
      <TopNToggle
        totalRows={filtered.length}
        showAll={showAll}
        onToggle={() => setShowAll((v) => !v)}
      />
      <div
        ref={parentRef}
        className="max-h-[60vh] overflow-auto"
        style={shouldVirtualize ? { height: '60vh' } : undefined}
      >
        <table className="w-full table-fixed text-left">
          <thead className="sticky top-0" style={{ background: 'var(--surface-base)' }}>
            <tr>
              <th className="ui-table-head w-12">#</th>
              <th className="ui-table-head">URL</th>
            </tr>
          </thead>
          {shouldVirtualize ? (
            <VirtualTableBody
              rows={displayRows}
              parentRef={parentRef}
              renderRow={(url, i) => (
                <>
                  <td className="ui-table-cell ui-table-cell-muted tabular-nums">{i + 1}</td>
                  <td className="ui-table-cell truncate max-w-[600px]">
                    <UrlCell url={url} />
                  </td>
                </>
              )}
            />
          ) : (
            <tbody>
              {displayRows.map((url, i) => (
                <tr
                  key={url}
                  className="border-b border-[var(--border-subtle)] hover:bg-[var(--surface-float)] transition-colors animate-fade-in-up"
                  style={{ animationDelay: `${i * 20}ms` }}
                >
                  <td className="ui-table-cell ui-table-cell-muted tabular-nums">{i + 1}</td>
                  <td className="ui-table-cell truncate max-w-[600px]">
                    <UrlCell url={url} />
                  </td>
                </tr>
              ))}
            </tbody>
          )}
        </table>
      </div>
    </div>
  )
}
