'use client'

import { useVirtualizer } from '@tanstack/react-virtual'
import { AlertCircle, Library, RefreshCw, Search } from 'lucide-react'
import { useSearchParams } from 'next/navigation'
import { useEffect, useMemo, useRef, useState } from 'react'
import { apiFetch } from '@/lib/api-fetch'
import type { SourcesResult } from '@/lib/result-types'

interface ApiResponse {
  ok: boolean
  data?: SourcesResult
  error?: string
}

export function SourcesDashboard() {
  const searchParams = useSearchParams()
  const [data, setData] = useState<SourcesResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [spinning, setSpinning] = useState(false)
  const [updatedAt, setUpdatedAt] = useState<Date | null>(null)
  const [query, setQuery] = useState(() => searchParams.get('q') ?? '')
  const parentRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    setQuery(searchParams.get('q') ?? '')
  }, [searchParams])

  async function load(isManual = false) {
    if (isManual) setSpinning(true)
    setError(null)
    try {
      const res = await apiFetch('/api/cortex/sources')
      const json = (await res.json()) as ApiResponse
      if (!json.ok) throw new Error(json.error ?? 'Unknown error')
      setData(json.data ?? null)
      setUpdatedAt(new Date())
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
      setSpinning(false)
    }
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: load is stable within the render; deps would cause double-fetch on mount
  useEffect(() => {
    void load()
  }, [])

  const rows = useMemo(() => {
    if (!data) return []
    const entries = Object.entries(data).sort((a, b) => b[1] - a[1])
    if (!query.trim()) return entries
    const q = query.toLowerCase()
    return entries.filter(([url]) => url.toLowerCase().includes(q))
  }, [data, query])

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 36,
    overscan: 10,
  })

  return (
    <div className="animate-fade-in flex h-[calc(100vh-8rem)] flex-col gap-4">
      {/* Header */}
      <div className="flex items-center gap-3">
        <Library className="size-4 text-[var(--axon-primary)]" />
        <h1 className="text-[18px] font-bold tracking-tight text-[var(--text-primary)]">Sources</h1>
        {data && (
          <span className="rounded-full bg-[rgba(135,175,255,0.12)] px-2 py-0.5 text-[10px] font-semibold text-[var(--axon-primary)]">
            {Object.keys(data).length.toLocaleString()} URLs
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

      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-[var(--text-dim)]" />
        <input
          type="text"
          placeholder="Filter URLs…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--surface-base)] py-2 pl-8 pr-3 text-[12px] text-[var(--text-primary)] placeholder:text-[var(--text-dim)] focus:border-[rgba(135,175,255,0.4)] focus:outline-none"
        />
      </div>

      {loading && (
        <div className="space-y-1.5">
          {Array.from({ length: 8 }).map((_, i) => (
            <div
              key={i}
              className="h-8 animate-pulse rounded-lg border border-[var(--border-subtle)] bg-[var(--surface-base)]"
            />
          ))}
        </div>
      )}

      {!loading && error && (
        <div className="flex items-start gap-3 rounded-xl border border-[rgba(251,113,133,0.3)] bg-[rgba(251,113,133,0.08)] px-4 py-3 text-[12px] text-[#fb7185]">
          <AlertCircle className="mt-0.5 size-4 flex-shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {!loading && !error && data && (
        <div
          ref={parentRef}
          className="flex-1 overflow-auto rounded-xl border border-[var(--border-subtle)]"
        >
          <div style={{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }}>
            {virtualizer.getVirtualItems().map((vItem) => {
              const [url, count] = rows[vItem.index] ?? ['', 0]
              return (
                <div
                  key={vItem.key}
                  data-index={vItem.index}
                  ref={virtualizer.measureElement}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    transform: `translateY(${vItem.start}px)`,
                  }}
                  className="flex items-center gap-3 border-b border-[var(--border-subtle)] px-4 py-2 hover:bg-[var(--surface-float)]"
                >
                  <span className="flex-1 truncate font-mono text-[11px] text-[var(--text-secondary)]">
                    {url}
                  </span>
                  <span className="flex-shrink-0 rounded bg-[rgba(135,175,255,0.1)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--axon-primary)]">
                    {count}
                  </span>
                </div>
              )
            })}
          </div>
        </div>
      )}

      {!loading && !error && rows.length === 0 && query && (
        <p className="text-center text-[12px] text-[var(--text-dim)]">No results for "{query}"</p>
      )}
    </div>
  )
}
