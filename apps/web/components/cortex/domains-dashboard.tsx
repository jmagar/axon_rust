'use client'

import { AlertCircle, Globe, RefreshCw } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { apiFetch } from '@/lib/api-fetch'
import type { DomainsPagedResult, DomainsResult } from '@/lib/result-types'

interface ApiResponse {
  ok: boolean
  data?: DomainsResult
  error?: string
}

function isDomainsPagedResult(data: DomainsResult): data is DomainsPagedResult {
  if (!('domains' in data)) return false
  if (!Array.isArray(data.domains)) return false
  return data.domains.every(
    (row) =>
      !!row &&
      typeof row === 'object' &&
      'domain' in row &&
      typeof row.domain === 'string' &&
      'vectors' in row,
  )
}

function parseCount(v: number | [number, number]): { urls: number; vectors: number } {
  if (Array.isArray(v)) return { urls: v[0], vectors: v[1] }
  return { urls: 0, vectors: v }
}

function normalizeDomains(
  data: DomainsResult | null,
): Array<{ domain: string; urls: number; vectors: number }> {
  if (!data) return []

  if (isDomainsPagedResult(data)) {
    return data.domains
      .map((row) => ({
        domain: row.domain,
        urls: Number(row.urls ?? 0) || 0,
        vectors: Number(row.vectors) || 0,
      }))
      .sort((a, b) => b.vectors - a.vectors)
  }

  return Object.entries(data)
    .map(([domain, v]) => ({ domain, ...parseCount(v) }))
    .sort((a, b) => b.vectors - a.vectors)
}

export function DomainsDashboard() {
  const [data, setData] = useState<DomainsResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [spinning, setSpinning] = useState(false)
  const [updatedAt, setUpdatedAt] = useState<Date | null>(null)

  async function load(isManual = false) {
    if (isManual) setSpinning(true)
    setError(null)
    try {
      const res = await apiFetch('/api/cortex/domains')
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
    return normalizeDomains(data)
  }, [data])

  const maxVectors = rows[0]?.vectors ?? 1

  return (
    <div className="animate-fade-in space-y-4">
      <div className="flex items-center gap-3">
        <Globe className="size-4 text-[var(--axon-primary)]" />
        <h1 className="text-[18px] font-bold tracking-tight text-[var(--text-primary)]">Domains</h1>
        {data && (
          <span className="rounded-full bg-[rgba(135,175,255,0.12)] px-2 py-0.5 text-[10px] font-semibold text-[var(--axon-primary)]">
            {rows.length} domains
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
        <div className="space-y-2">
          {Array.from({ length: 8 }).map((_, i) => (
            <div
              key={i}
              className="h-10 animate-pulse rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-base)]"
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
        <div className="overflow-hidden rounded-xl border border-[var(--border-subtle)]">
          <table className="w-full">
            <thead>
              <tr className="border-b border-[var(--border-subtle)] bg-[var(--surface-base)]">
                <th className="px-4 py-2.5 text-left text-[9px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
                  Domain
                </th>
                <th className="w-40 px-4 py-2.5 text-right text-[9px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
                  Vectors
                </th>
                <th className="w-52 px-4 py-2.5" />
              </tr>
            </thead>
            <tbody>
              {rows.map(({ domain, urls, vectors }) => {
                const barPct = maxVectors > 0 ? (vectors / maxVectors) * 100 : 0
                return (
                  <tr
                    key={domain}
                    className="border-b border-[var(--border-subtle)] last:border-0 hover:bg-[var(--surface-float)]"
                  >
                    <td className="px-4 py-2.5">
                      <a
                        href={`/cortex/sources?q=${encodeURIComponent(domain)}`}
                        className="font-mono text-[11px] text-[var(--axon-primary)] hover:underline"
                      >
                        {domain}
                      </a>
                      {urls > 0 && (
                        <span className="ml-2 text-[10px] text-[var(--text-dim)]">{urls} URLs</span>
                      )}
                    </td>
                    <td className="px-4 py-2.5 text-right font-mono text-[11px] text-[var(--text-secondary)]">
                      {vectors.toLocaleString()}
                    </td>
                    <td className="px-4 py-2.5">
                      <div className="h-1.5 w-full rounded-full bg-[var(--surface-float)]">
                        <div
                          className="h-1.5 rounded-full bg-[rgba(135,175,255,0.5)]"
                          style={{ width: `${barPct}%` }}
                        />
                      </div>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}

      {!loading && !error && !data && (
        <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-base)] px-4 py-3 text-[12px] text-[var(--text-dim)]">
          Domain stats are not available yet. Try Refresh.
        </div>
      )}
    </div>
  )
}
