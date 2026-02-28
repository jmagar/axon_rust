'use client'

import { useMemo, useState } from 'react'
import type { RetrieveResult, SourcesResult, StatusResult, SuggestResult } from '@/lib/result-types'
import { FilterInput, fmtNum, StatusBadge, UrlCell } from './table-primitives'

// ---------------------------------------------------------------------------
// Status table (job queues)
// ---------------------------------------------------------------------------

export function StatusTable({ data }: { data: StatusResult }) {
  const queues = [
    { label: 'Crawl Jobs', jobs: data.local_crawl_jobs ?? [] },
    { label: 'Extract Jobs', jobs: data.local_extract_jobs ?? [] },
    { label: 'Embed Jobs', jobs: data.local_embed_jobs ?? [] },
    { label: 'Ingest Jobs', jobs: data.local_ingest_jobs ?? [] },
  ].filter((q) => q.jobs.length > 0)

  if (queues.length === 0) {
    return <div className="text-sm text-[var(--text-muted)]">No active jobs</div>
  }

  return (
    <div className="space-y-4">
      {queues.map((queue) => (
        <div key={queue.label}>
          <div className="ui-label mb-1.5">
            {queue.label} ({queue.jobs.length})
          </div>
          <div className="overflow-auto">
            <table className="ui-table-dense">
              <thead>
                <tr>
                  <th className="ui-table-head">ID</th>
                  <th className="ui-table-head">URL</th>
                  <th className="ui-table-head">Status</th>
                </tr>
              </thead>
              <tbody>
                {queue.jobs.map((job) => (
                  <tr
                    key={job.id}
                    className="border-b border-[var(--border-subtle)] hover:bg-[var(--surface-float)]"
                  >
                    <td className="ui-table-cell ui-table-cell-muted">{job.id.slice(0, 8)}</td>
                    <td className="ui-table-cell max-w-[300px] truncate">
                      {job.url ? (
                        <UrlCell url={job.url} />
                      ) : (
                        <span className="text-[var(--text-dim)]">--</span>
                      )}
                    </td>
                    <td className="ui-table-cell">
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

export function SuggestTable({ data }: { data: SuggestResult }) {
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
      <div className="ui-meta mb-3 flex flex-wrap gap-4">
        <span>
          Collection: <span className="text-[var(--axon-primary)]">{data.collection}</span>
        </span>
        <span>
          Indexed:{' '}
          <span className="text-[var(--axon-primary)]">{fmtNum(data.indexed_urls_count)}</span>
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
        <table className="ui-table-dense">
          <thead className="sticky top-0" style={{ background: 'var(--surface-base)' }}>
            <tr>
              <th className="ui-table-head">URL</th>
              <th className="ui-table-head">Reason</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((s) => (
              <tr
                key={s.url}
                className="border-b border-[var(--border-subtle)] hover:bg-[var(--surface-float)]"
              >
                <td className="ui-table-cell max-w-[350px] truncate pr-4">
                  <UrlCell url={s.url} />
                </td>
                <td className="ui-table-cell ui-table-cell-muted">{s.reason}</td>
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

export function RetrieveView({ data }: { data: RetrieveResult }) {
  return (
    <div>
      <div className="ui-meta mb-3 flex flex-wrap gap-4">
        <span>
          URL: <UrlCell url={data.url} />
        </span>
        <span>
          Chunks: <span className="text-[var(--axon-primary)]">{fmtNum(data.chunks)}</span>
        </span>
      </div>
      <pre
        className="max-h-[55vh] overflow-auto whitespace-pre-wrap rounded-lg border border-[var(--border-subtle)] p-3 ui-mono text-[var(--text-secondary)]"
        style={{ background: 'var(--surface-elevated)' }}
      >
        {data.content}
      </pre>
    </div>
  )
}
