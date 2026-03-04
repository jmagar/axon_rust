'use client'

import { AlertCircle, BarChart2, RefreshCw } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { apiFetch } from '@/lib/api-fetch'
import type { StatsResult } from '@/lib/result-types'

interface ApiResponse {
  ok: boolean
  data?: StatsResult
  error?: string
}

// ── Big metric card ────────────────────────────────────────────────────────────

function MetricCard({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="flex flex-col gap-1 rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-base)] p-4">
      <span className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
        {label}
      </span>
      <span className="font-mono text-[22px] font-bold tabular-nums text-[var(--axon-primary)]">
        {typeof value === 'number' ? value.toLocaleString() : value}
      </span>
    </div>
  )
}

// ── Main dashboard ────────────────────────────────────────────────────────────

export function StatsDashboard() {
  const [data, setData] = useState<StatsResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [spinning, setSpinning] = useState(false)
  const [updatedAt, setUpdatedAt] = useState<Date | null>(null)
  const abortRef = useRef<AbortController | null>(null)

  async function load(isManual = false) {
    abortRef.current?.abort()
    const controller = new AbortController()
    abortRef.current = controller
    if (isManual) setSpinning(true)
    setError(null)
    try {
      const res = await apiFetch('/api/cortex/stats', { signal: controller.signal })
      const json = (await res.json()) as ApiResponse
      if (!json.ok) throw new Error(json.error ?? 'Unknown error')
      if (abortRef.current !== controller) return
      setData(json.data ?? null)
      setUpdatedAt(new Date())
    } catch (err) {
      if (abortRef.current !== controller) return
      if (err instanceof Error && err.name === 'AbortError') return
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      if (abortRef.current === controller) {
        setLoading(false)
        setSpinning(false)
      }
    }
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: load intentionally captured at mount; abortRef cleanup handles unmount race
  useEffect(() => {
    void load()
    const id = setInterval(() => void load(), 30_000)
    return () => {
      clearInterval(id)
      abortRef.current?.abort()
    }
  }, [])

  return (
    <div className="animate-fade-in space-y-4">
      <div className="flex items-center gap-3">
        <BarChart2 className="size-4 text-[var(--axon-primary)]" />
        <h1 className="text-[18px] font-bold tracking-tight text-[var(--text-primary)]">Stats</h1>
        {data && (
          <span
            className={`rounded-full px-2 py-0.5 text-[10px] font-semibold ${
              data.status === 'green'
                ? 'bg-[rgba(52,211,153,0.12)] text-[#34d399]'
                : 'bg-[rgba(251,191,36,0.12)] text-[#fbbf24]'
            }`}
          >
            {data.status}
          </span>
        )}
        <div className="flex-1" />
        {updatedAt && (
          <span className="text-[10px] text-[var(--text-dim)]">
            Updated {updatedAt.toLocaleTimeString()}
          </span>
        )}
        <button
          type="button"
          onClick={() => void load(true)}
          disabled={loading || spinning}
          className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[11px] font-medium text-[var(--text-dim)] transition-colors hover:bg-[var(--surface-float)] hover:text-[var(--axon-primary)] disabled:opacity-40"
        >
          <RefreshCw className={`size-3.5 ${spinning ? 'animate-spin' : ''}`} />
          Refresh
        </button>
      </div>

      {loading && (
        <div className="space-y-3">
          <div className="grid grid-cols-3 gap-3">
            {Array.from({ length: 6 }).map((_, i) => (
              <div
                key={i}
                className="h-20 animate-pulse rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-base)]"
              />
            ))}
          </div>
        </div>
      )}

      {!loading && error && (
        <div className="flex items-start gap-3 rounded-xl border border-[rgba(251,113,133,0.3)] bg-[rgba(251,113,133,0.08)] px-4 py-3 text-[12px] text-[#fb7185]">
          <AlertCircle className="mt-0.5 size-4 flex-shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {!loading && !error && data && (
        <>
          {/* Metric grid */}
          <div className="grid grid-cols-3 gap-3">
            <MetricCard label="Vectors" value={data.indexed_vectors_count} />
            <MetricCard label="Points" value={data.points_count} />
            <MetricCard label="Docs (est.)" value={data.docs_embedded_estimate} />
            <MetricCard label="Avg chunks/doc" value={(data.avg_chunks_per_doc ?? 0).toFixed(1)} />
            <MetricCard label="Dimension" value={data.dimension} />
            <MetricCard label="Segments" value={data.segments_count} />
          </div>

          {/* Collection info */}
          <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-base)] px-4 py-3">
            <div className="flex items-center gap-3 text-[12px]">
              <span className="font-semibold text-[var(--text-dim)]">Collection</span>
              <span className="font-mono text-[var(--text-primary)]">{data.collection}</span>
              <span className="font-semibold text-[var(--text-dim)]">Distance</span>
              <span className="font-mono text-[var(--text-primary)]">{data.distance}</span>
            </div>
          </div>

          {/* Payload fields */}
          {(data.payload_fields ?? []).length > 0 && (
            <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-base)] p-4">
              <p className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
                Payload Fields
              </p>
              <div className="flex flex-wrap gap-1.5">
                {(data.payload_fields ?? []).map((f) => (
                  <span
                    key={f}
                    className="rounded-md bg-[rgba(135,175,255,0.1)] px-2 py-0.5 font-mono text-[10px] text-[var(--axon-primary)]"
                  >
                    {f}
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Command counts */}
          {Object.keys(data.counts ?? {}).length > 0 && (
            <div className="overflow-hidden rounded-xl border border-[var(--border-subtle)]">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-[var(--border-subtle)] bg-[var(--surface-base)]">
                    <th className="px-4 py-2 text-left text-[9px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
                      Command
                    </th>
                    <th className="px-4 py-2 text-right text-[9px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
                      Count
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {Object.entries(data.counts ?? {})
                    .sort((a, b) => b[1] - a[1])
                    .map(([cmd, count]) => (
                      <tr
                        key={cmd}
                        className="border-b border-[var(--border-subtle)] last:border-0 hover:bg-[var(--surface-float)]"
                      >
                        <td className="px-4 py-2 font-mono text-[11px] text-[var(--text-secondary)]">
                          {cmd}
                        </td>
                        <td className="px-4 py-2 text-right font-mono text-[11px] text-[var(--axon-primary)]">
                          {count.toLocaleString()}
                        </td>
                      </tr>
                    ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
    </div>
  )
}
