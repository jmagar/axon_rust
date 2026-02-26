'use client'

import type { DoctorResult, DoctorServiceStatus } from '@/lib/result-types'
import { fmtMs as formatDurationMs } from './shared'

function fmtProbeMs(ms: number | undefined): string {
  if (ms === undefined) return '--'
  return formatDurationMs(ms)
}

function cleanServiceMeta(text: string | undefined): string | undefined {
  if (!text) return undefined
  const cleaned = text
    .replace(/\bhttp\s+\d{3}\b/gi, '')
    .replace(/\bstatus\s*:\s*\d{3}\b/gi, '')
    .replace(/\s{2,}/g, ' ')
    .trim()
  return cleaned.length > 0 ? cleaned : undefined
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="mb-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--axon-accent-blue-strong)]">
      {children}
    </h3>
  )
}

function MetricTile({
  label,
  value,
  accent,
}: {
  label: string
  value: string
  accent: 'blue' | 'pink' | 'green' | 'orange'
}) {
  const colorClass =
    accent === 'pink'
      ? 'text-[var(--axon-accent-pink)] border-[rgba(175,215,255,0.28)] bg-[rgba(175,215,255,0.08)]'
      : accent === 'green'
        ? 'text-[var(--axon-success)] border-[rgba(130,217,160,0.28)] bg-[var(--axon-success-bg)]'
        : accent === 'orange'
          ? 'text-[var(--axon-warning)] border-[rgba(255,192,134,0.28)] bg-[var(--axon-warning-bg)]'
          : 'text-[var(--axon-accent-blue)] border-[rgba(255,135,175,0.24)] bg-[rgba(255,135,175,0.08)]'

  return (
    <div className={`rounded-lg border px-3 py-2.5 ${colorClass}`}>
      <div className="text-[10px] uppercase tracking-wider text-[var(--axon-text-muted)]">
        {label}
      </div>
      <div className="mt-1 font-mono text-[16px] font-semibold leading-none">{value}</div>
    </div>
  )
}

function StatusPill({ ok }: { ok: boolean }) {
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide ${
        ok
          ? 'bg-[var(--axon-success-bg)] text-[var(--axon-success)]'
          : 'bg-[rgba(175,215,255,0.2)] text-[var(--axon-accent-pink)]'
      }`}
    >
      <span className="text-[9px]">{'\u25CF'}</span>
      {ok ? 'ok' : 'fail'}
    </span>
  )
}

function ServiceRows({ entries }: { entries: Array<[string, DoctorServiceStatus]> }) {
  return (
    <div className="rounded-lg border border-[rgba(255,135,175,0.14)] bg-[rgba(9,16,34,0.55)]">
      <div className="grid grid-cols-[110px_130px_1fr] gap-3 border-b border-[rgba(255,135,175,0.12)] px-3 py-2 text-[10px] uppercase tracking-wider text-[var(--axon-text-dim)]">
        <span>Status</span>
        <span>Latency</span>
        <span>Service</span>
      </div>
      <div className="max-h-[48vh] overflow-auto">
        {entries.map(([name, svc]) => {
          const detail = cleanServiceMeta(svc.detail)
          const summary = cleanServiceMeta(svc.summary)
          return (
            <div
              key={name}
              className="border-b border-[rgba(255,135,175,0.08)] px-3 py-2.5 last:border-b-0"
            >
              <div className="grid grid-cols-[110px_130px_1fr] gap-3">
                <div className="pt-0.5">
                  <StatusPill ok={svc.ok} />
                </div>
                <div className="pt-1 font-mono text-[10px] text-[var(--axon-text-secondary)]">
                  {fmtProbeMs(svc.latency_ms)}
                </div>
                <div>
                  <div className="text-[12px] font-semibold text-[var(--axon-text-secondary)]">
                    {name}
                  </div>
                  {svc.model && (
                    <div className="mt-0.5 break-words text-[10px] text-[var(--axon-accent-blue)]">
                      {svc.model}
                    </div>
                  )}
                  {svc.url && (
                    <div className="mt-0.5 break-all font-mono text-[10px] text-[var(--axon-accent-blue)]">
                      {svc.url}
                    </div>
                  )}
                  {detail && (
                    <div className="mt-1 whitespace-pre-wrap break-words text-[10px] text-[var(--axon-text-muted)]">
                      {detail}
                    </div>
                  )}
                  {summary && (
                    <div className="mt-1 whitespace-pre-wrap break-words text-[10px] text-[var(--axon-text-muted)]">
                      {summary}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

export function DoctorReport({ data }: { data: DoctorResult }) {
  const serviceEntries = Object.entries(data.services).sort(([, a], [, b]) =>
    Number(a.ok) === Number(b.ok) ? 0 : a.ok ? 1 : -1,
  )
  const pipelineEntries = Object.entries(data.pipelines ?? {})
  const queueEntries = Object.entries(data.queue_names ?? {})

  const healthyServices = serviceEntries.filter(([, svc]) => svc.ok).length
  const unhealthyServices = serviceEntries.length - healthyServices
  const healthyPipelines = pipelineEntries.filter(([, ok]) => ok).length
  const unhealthyPipelines = pipelineEntries.length - healthyPipelines
  const observedAt = data.observed_at_utc ? new Date(data.observed_at_utc).toLocaleString() : null
  const timingRows = [
    ['crawl report', data.timing_ms?.crawl_report],
    ['extract report', data.timing_ms?.extract_report],
    ['embed report', data.timing_ms?.embed_report],
    ['ingest report', data.timing_ms?.ingest_report],
    ['stale/pending query', data.timing_ms?.stale_pending],
  ] as const

  return (
    <div className="space-y-4">
      <div
        className="rounded-xl border px-3 py-3"
        style={{
          borderColor: data.all_ok ? 'rgba(135,215,135,0.24)' : 'rgba(175,215,255,0.34)',
          background:
            'linear-gradient(120deg, rgba(14,24,44,0.74) 0%, rgba(11,20,38,0.92) 52%, rgba(17,26,48,0.74) 100%)',
        }}
      >
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <span
            className={`inline-block size-2.5 rounded-full ${
              data.all_ok
                ? 'bg-[var(--axon-success)] shadow-[0_0_8px_rgba(130,217,160,0.65)]'
                : 'bg-[var(--axon-accent-pink)] shadow-[0_0_8px_rgba(175,215,255,0.65)]'
            }`}
          />
          <span className="text-[12px] font-semibold text-[var(--axon-text-secondary)]">
            {data.all_ok ? 'System Health: Stable' : 'System Health: Attention Needed'}
          </span>
          {observedAt && (
            <span className="text-[10px] text-[var(--axon-text-muted)]">Observed {observedAt}</span>
          )}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <MetricTile label="Services Up" value={`${healthyServices}`} accent="green" />
        <MetricTile label="Services Down" value={`${unhealthyServices}`} accent="pink" />
        <MetricTile label="Pending Jobs" value={`${data.pending_jobs}`} accent="blue" />
        <MetricTile label="Stale Jobs" value={`${data.stale_jobs}`} accent="orange" />
      </div>

      <div className="grid gap-4 lg:grid-cols-[1.7fr_1fr]">
        <div>
          <SectionHeader>Services</SectionHeader>
          <ServiceRows entries={serviceEntries} />
        </div>

        <div className="space-y-4">
          <div className="rounded-lg border border-[rgba(255,135,175,0.15)] p-3 bg-[rgba(9,16,34,0.55)]">
            <SectionHeader>Pipelines</SectionHeader>
            <div className="mb-3 grid grid-cols-2 gap-3">
              <MetricTile label="Up" value={`${healthyPipelines}`} accent="green" />
              <MetricTile label="Down" value={`${unhealthyPipelines}`} accent="pink" />
            </div>
            <div className="space-y-1.5">
              {pipelineEntries.map(([name, ok]) => (
                <div
                  key={name}
                  className="flex items-center justify-between rounded-md border border-[rgba(255,135,175,0.08)] bg-[rgba(8,14,30,0.4)] px-2 py-1.5 text-[11px]"
                >
                  <span className="text-[var(--axon-text-secondary)]">{name}</span>
                  <span
                    className={ok ? 'text-[var(--axon-success)]' : 'text-[var(--axon-accent-pink)]'}
                  >
                    {ok ? 'up' : 'down'}
                  </span>
                </div>
              ))}
            </div>
          </div>

          {queueEntries.length > 0 && (
            <div className="rounded-lg border border-[rgba(255,135,175,0.15)] p-3 bg-[rgba(9,16,34,0.55)]">
              <SectionHeader>Queue Names</SectionHeader>
              <div className="space-y-1.5 font-mono text-[11px]">
                {queueEntries.map(([key, val]) => (
                  <div
                    key={key}
                    className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.5fr)] gap-3 rounded-md border border-[rgba(255,135,175,0.08)] bg-[rgba(8,14,30,0.4)] px-2 py-1.5"
                  >
                    <span className="break-words text-[var(--axon-text-dim)]">{key}</span>
                    <span className="break-all text-right text-[var(--axon-accent-blue)]">
                      {val}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {data.browser_runtime && (
            <div className="rounded-lg border border-[rgba(255,135,175,0.15)] p-3 bg-[rgba(9,16,34,0.55)]">
              <SectionHeader>Browser Runtime</SectionHeader>
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-[var(--axon-text-dim)]">Selection</span>
                <span className="font-mono text-[var(--axon-accent-blue)]">
                  {data.browser_runtime.selection}
                </span>
              </div>
            </div>
          )}

          {data.timing_ms && (
            <div className="rounded-lg border border-[rgba(255,135,175,0.15)] p-3 bg-[rgba(9,16,34,0.55)]">
              <SectionHeader>Probe Timing</SectionHeader>
              <div className="space-y-1.5 font-mono text-[11px]">
                {timingRows.map(([label, value]) => (
                  <div key={label} className="flex justify-between">
                    <span className="text-[var(--axon-text-dim)]">{label}</span>
                    <span className="text-[var(--axon-accent-blue)]">{fmtProbeMs(value)}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
