'use client'

import type { NormalizedResult, QueryResult } from '@/lib/result-types'

interface CardsRendererProps {
  result: NormalizedResult
}

export function CardsRenderer({ result }: CardsRendererProps) {
  if (result.type !== 'query') return null
  return <QueryCards results={result.data} />
}

// ---------------------------------------------------------------------------
// Score color — green > 0.7, yellow > 0.4, red < 0.4
// ---------------------------------------------------------------------------

function scoreColor(score: number): string {
  if (score >= 0.7) return 'var(--axon-success)'
  if (score >= 0.4) return 'var(--axon-warning)'
  return 'var(--axon-accent-pink)'
}

function scoreBg(score: number): string {
  if (score >= 0.7) return 'var(--axon-success-bg)'
  if (score >= 0.4) return 'var(--axon-warning-bg)'
  return 'var(--axon-danger-bg)'
}

// ---------------------------------------------------------------------------
// Query result cards
// ---------------------------------------------------------------------------

function QueryCards({ results }: { results: QueryResult[] }) {
  if (results.length === 0) {
    return <div className="text-sm text-[var(--axon-text-muted)]">No results</div>
  }

  return (
    <div className="space-y-2.5">
      {results.map((r) => (
        <div
          key={`${r.rank}-${r.url}`}
          className="rounded-lg border border-[rgba(255,135,175,0.08)] p-3 transition-colors hover:border-[rgba(255,135,175,0.15)]"
          style={{ background: 'rgba(10, 18, 35, 0.4)' }}
        >
          {/* Header: rank badge + score + URL */}
          <div className="mb-2 flex items-start gap-2.5">
            {/* Rank badge */}
            <span
              className="flex size-6 shrink-0 items-center justify-center rounded-full text-[10px] font-bold"
              style={{
                background: 'rgba(135, 175, 255, 0.15)',
                color: 'var(--axon-accent-blue-strong)',
              }}
            >
              {r.rank}
            </span>

            <div className="min-w-0 flex-1">
              {/* URL */}
              <a
                href={r.url}
                target="_blank"
                rel="noopener noreferrer"
                className="block truncate text-[13px] font-medium text-[var(--axon-accent-blue-strong)] transition-colors hover:text-[var(--axon-accent-blue)] hover:underline"
              >
                {r.url}
              </a>

              {/* Source tag if different from URL */}
              {r.source && r.source !== r.url && (
                <span className="mt-0.5 block truncate text-[11px] text-[var(--axon-text-subtle)]">
                  {r.source}
                </span>
              )}
            </div>

            {/* Score badges */}
            <div className="flex shrink-0 gap-1.5">
              <span
                className="rounded-md px-1.5 py-0.5 font-mono text-[10px] font-semibold"
                style={{
                  color: scoreColor(r.score),
                  background: scoreBg(r.score),
                }}
              >
                {r.score.toFixed(3)}
              </span>
              {r.rerank_score !== undefined && (
                <span
                  className="rounded-md px-1.5 py-0.5 font-mono text-[10px] font-semibold"
                  style={{
                    color: scoreColor(r.rerank_score),
                    background: scoreBg(r.rerank_score),
                  }}
                  title="Rerank score"
                >
                  RR {r.rerank_score.toFixed(3)}
                </span>
              )}
            </div>
          </div>

          {/* Snippet */}
          <p className="text-[12px] leading-relaxed text-[var(--axon-text-muted)]">{r.snippet}</p>
        </div>
      ))}
    </div>
  )
}
