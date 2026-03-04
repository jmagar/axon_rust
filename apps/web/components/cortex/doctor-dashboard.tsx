'use client'

import { AlertCircle, CheckCircle2, RefreshCw, Stethoscope, XCircle } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { apiFetch } from '@/lib/api-fetch'
import type { DoctorResult, DoctorServiceStatus } from '@/lib/result-types'

// ── Service card ──────────────────────────────────────────────────────────────

function ServiceCard({ name, svc }: { name: string; svc: DoctorServiceStatus }) {
  return (
    <div className="flex flex-col gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-base)] p-4">
      <div className="flex items-center gap-2">
        {svc.ok ? (
          <span className="relative flex size-2.5">
            <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[#34d399] opacity-60" />
            <span className="relative inline-flex size-2.5 rounded-full bg-[#34d399]" />
          </span>
        ) : (
          <span className="size-2.5 rounded-full bg-[#fb7185]" />
        )}
        <span className="text-[13px] font-semibold capitalize text-[var(--text-primary)]">
          {name}
        </span>
        {svc.latency_ms !== undefined && (
          <span className="ml-auto font-mono text-[10px] text-[var(--text-dim)]">
            {svc.latency_ms}ms
          </span>
        )}
      </div>
      {svc.url && <p className="truncate text-[10px] text-[var(--text-dim)]">{svc.url}</p>}
      {svc.model && <p className="text-[10px] text-[var(--text-dim)]">Model: {svc.model}</p>}
      {svc.detail && <p className="text-[10px] text-[#fb7185]">{svc.detail}</p>}
    </div>
  )
}

// ── Main dashboard ────────────────────────────────────────────────────────────

interface ApiResponse {
  ok: boolean
  data?: DoctorResult
  error?: string
}

export function DoctorDashboard() {
  const [data, setData] = useState<DoctorResult | null>(null)
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
      const res = await apiFetch('/api/cortex/doctor', { signal: controller.signal })
      const json = (await res.json()) as ApiResponse
      if (!json.ok) throw new Error(json.error ?? 'Unknown error')
      if (abortRef.current !== controller) return
      setData(json.data ?? null)
      setUpdatedAt(new Date())
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') return
      if (abortRef.current !== controller) return
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      if (abortRef.current === controller) {
        abortRef.current = null
        setLoading(false)
        setSpinning(false)
      }
    }
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: load intentionally captured at mount; abortRef cleanup handles unmount race
  useEffect(() => {
    void load()
    const id = setInterval(() => void load(), 15_000)
    return () => {
      clearInterval(id)
      abortRef.current?.abort()
    }
  }, [])

  const allOk = data?.all_ok ?? false
  const services = data ? Object.entries(data.services ?? {}) : []
  const pipelines = data ? Object.entries(data.pipelines ?? {}) : []

  return (
    <div className="animate-fade-in space-y-4">
      <div className="flex items-center gap-3">
        <Stethoscope className="size-4 text-[var(--axon-primary)]" />
        <h1 className="text-[18px] font-bold tracking-tight text-[var(--text-primary)]">Doctor</h1>
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
          <div className="h-14 animate-pulse rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-base)]" />
          <div className="grid grid-cols-2 gap-3">
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
          {/* Health banner */}
          <div
            className={`flex items-center gap-3 rounded-xl border px-4 py-3 ${
              allOk
                ? 'border-[rgba(52,211,153,0.3)] bg-[rgba(52,211,153,0.08)]'
                : 'border-[rgba(251,113,133,0.3)] bg-[rgba(251,113,133,0.08)]'
            }`}
          >
            {allOk ? (
              <CheckCircle2 className="size-5 text-[#34d399]" />
            ) : (
              <XCircle className="size-5 text-[#fb7185]" />
            )}
            <div>
              <p
                className={`text-[13px] font-semibold ${allOk ? 'text-[#34d399]' : 'text-[#fb7185]'}`}
              >
                {allOk
                  ? 'All systems operational'
                  : `${services.filter(([, s]) => !s.ok).length} service(s) down`}
              </p>
              {data.stale_jobs > 0 && (
                <p className="text-[11px] text-[var(--text-dim)]">
                  {data.stale_jobs} stale job(s) · {data.pending_jobs} pending
                </p>
              )}
            </div>
          </div>

          {/* Service grid */}
          <div className="grid grid-cols-2 gap-3">
            {services.map(([name, svc]) => (
              <ServiceCard key={name} name={name} svc={svc} />
            ))}
          </div>

          {/* Pipelines */}
          {pipelines.length > 0 && (
            <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-base)] p-4">
              <p className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
                Pipelines
              </p>
              <div className="flex flex-wrap gap-1.5">
                {pipelines.map(([name, ok]) => (
                  <span
                    key={name}
                    className={`rounded-md px-2 py-0.5 text-[10px] font-semibold ${
                      ok
                        ? 'bg-[rgba(52,211,153,0.12)] text-[#34d399]'
                        : 'bg-[rgba(251,113,133,0.12)] text-[#fb7185]'
                    }`}
                  >
                    {name}
                  </span>
                ))}
              </div>
            </div>
          )}
        </>
      )}
    </div>
  )
}
