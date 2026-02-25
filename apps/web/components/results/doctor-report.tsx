'use client'

import type { DoctorResult, DoctorServiceStatus } from '@/lib/result-types'

function fmtMs(ms: number | undefined): string {
  if (ms === undefined) return '--'
  return `${ms} ms`
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
    <h3 className="mb-2 text-[9px] font-semibold uppercase tracking-wider text-[#8cc0ff]">
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
      ? 'text-[#ff87af] border-[rgba(255,135,175,0.28)] bg-[rgba(255,135,175,0.08)]'
      : accent === 'green'
        ? 'text-[#87d787] border-[rgba(135,215,135,0.24)] bg-[rgba(135,215,135,0.08)]'
        : accent === 'orange'
          ? 'text-[#ffb38a] border-[rgba(255,179,138,0.26)] bg-[rgba(255,179,138,0.08)]'
          : 'text-[#afd7ff] border-[rgba(175,215,255,0.24)] bg-[rgba(175,215,255,0.08)]'

  return (
    <div className={`rounded-lg border px-3 py-2.5 ${colorClass}`}>
      <div className="text-[9px] uppercase tracking-wider text-[#a4afc2]">{label}</div>
      <div className="mt-1 font-mono text-[16px] font-semibold leading-none">{value}</div>
    </div>
  )
}

function StatusPill({ ok }: { ok: boolean }) {
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[9px] font-semibold uppercase tracking-wide ${
        ok
          ? 'bg-[rgba(135,215,135,0.16)] text-[#87d787]'
          : 'bg-[rgba(255,135,175,0.2)] text-[#ff87af]'
      }`}
    >
      <span className="text-[8px]">{'\u25CF'}</span>
      {ok ? 'ok' : 'fail'}
    </span>
  )
}

function ServiceRows({ entries }: { entries: Array<[string, DoctorServiceStatus]> }) {
  return (
    <div className="rounded-lg border border-[rgba(175,215,255,0.14)] bg-[rgba(9,16,34,0.55)]">
      <div className="grid grid-cols-[110px_130px_1fr] gap-3 border-b border-[rgba(175,215,255,0.12)] px-3 py-2 text-[9px] uppercase tracking-wider text-[#8aa5c8]">
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
              className="border-b border-[rgba(175,215,255,0.08)] px-3 py-2.5 last:border-b-0"
            >
              <div className="grid grid-cols-[110px_130px_1fr] gap-3">
                <div className="pt-0.5">
                  <StatusPill ok={svc.ok} />
                </div>
                <div className="pt-1 font-mono text-[9px] text-[#dce6f0]">
                  {fmtMs(svc.latency_ms)}
                </div>
                <div>
                  <div className="text-[11px] font-semibold text-[#dce6f0]">{name}</div>
                  {svc.model && (
                    <div className="mt-0.5 break-words text-[9px] text-[#afd7ff]">{svc.model}</div>
                  )}
                  {svc.url && (
                    <div className="mt-0.5 break-all font-mono text-[9px] text-[#8fb6df]">
                      {svc.url}
                    </div>
                  )}
                  {detail && (
                    <div className="mt-1 whitespace-pre-wrap break-words text-[9px] text-[#9aa8bd]">
                      {detail}
                    </div>
                  )}
                  {summary && (
                    <div className="mt-1 whitespace-pre-wrap break-words text-[9px] text-[#9aa8bd]">
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

  return (
    <div className="space-y-4">
      <div
        className="rounded-xl border px-3 py-3"
        style={{
          borderColor: data.all_ok ? 'rgba(135,215,135,0.24)' : 'rgba(255,135,175,0.34)',
          background:
            'linear-gradient(120deg, rgba(14,24,44,0.74) 0%, rgba(11,20,38,0.92) 52%, rgba(17,26,48,0.74) 100%)',
        }}
      >
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <span
            className={`inline-block size-2.5 rounded-full ${
              data.all_ok
                ? 'bg-[#87d787] shadow-[0_0_8px_rgba(135,215,135,0.65)]'
                : 'bg-[#ff87af] shadow-[0_0_8px_rgba(255,135,175,0.65)]'
            }`}
          />
          <span className="text-[12px] font-semibold text-[#dce6f0]">
            {data.all_ok ? 'System Health: Stable' : 'System Health: Attention Needed'}
          </span>
          {observedAt && <span className="text-[9px] text-[#9aa8bd]">Observed {observedAt}</span>}
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
          <div className="rounded-lg border border-[rgba(175,215,255,0.15)] p-3 bg-[rgba(9,16,34,0.55)]">
            <SectionHeader>Pipelines</SectionHeader>
            <div className="mb-3 grid grid-cols-2 gap-3">
              <MetricTile label="Up" value={`${healthyPipelines}`} accent="green" />
              <MetricTile label="Down" value={`${unhealthyPipelines}`} accent="pink" />
            </div>
            <div className="space-y-1.5">
              {pipelineEntries.map(([name, ok]) => (
                <div
                  key={name}
                  className="flex items-center justify-between rounded-md border border-[rgba(175,215,255,0.08)] bg-[rgba(8,14,30,0.4)] px-2 py-1.5 text-[10px]"
                >
                  <span className="text-[#dce6f0]">{name}</span>
                  <span className={ok ? 'text-[#87d787]' : 'text-[#ff87af]'}>
                    {ok ? 'up' : 'down'}
                  </span>
                </div>
              ))}
            </div>
          </div>

          {queueEntries.length > 0 && (
            <div className="rounded-lg border border-[rgba(175,215,255,0.15)] p-3 bg-[rgba(9,16,34,0.55)]">
              <SectionHeader>Queue Names</SectionHeader>
              <div className="space-y-1.5 font-mono text-[10px]">
                {queueEntries.map(([key, val]) => (
                  <div
                    key={key}
                    className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.5fr)] gap-3 rounded-md border border-[rgba(175,215,255,0.08)] bg-[rgba(8,14,30,0.4)] px-2 py-1.5"
                  >
                    <span className="break-words text-[#87a1c2]">{key}</span>
                    <span className="break-all text-right text-[#afd7ff]">{val}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {data.browser_runtime && (
            <div className="rounded-lg border border-[rgba(175,215,255,0.15)] p-3 bg-[rgba(9,16,34,0.55)]">
              <SectionHeader>Browser Runtime</SectionHeader>
              <div className="flex items-center justify-between text-[10px]">
                <span className="text-[#87a1c2]">Selection</span>
                <span className="font-mono text-[#afd7ff]">{data.browser_runtime.selection}</span>
              </div>
            </div>
          )}

          {data.timing_ms && (
            <div className="rounded-lg border border-[rgba(175,215,255,0.15)] p-3 bg-[rgba(9,16,34,0.55)]">
              <SectionHeader>Probe Timing</SectionHeader>
              <div className="space-y-1.5 font-mono text-[10px]">
                <div className="flex justify-between">
                  <span className="text-[#87a1c2]">crawl report</span>
                  <span className="text-[#afd7ff]">{fmtMs(data.timing_ms.crawl_report)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[#87a1c2]">extract report</span>
                  <span className="text-[#afd7ff]">{fmtMs(data.timing_ms.extract_report)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[#87a1c2]">embed report</span>
                  <span className="text-[#afd7ff]">{fmtMs(data.timing_ms.embed_report)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[#87a1c2]">ingest report</span>
                  <span className="text-[#afd7ff]">{fmtMs(data.timing_ms.ingest_report)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[#87a1c2]">stale/pending query</span>
                  <span className="text-[#afd7ff]">{fmtMs(data.timing_ms.stale_pending)}</span>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
