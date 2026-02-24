'use client'

import { useState } from 'react'
import type {
  AskDiagnostics,
  AskResult,
  DebugResult,
  DoctorResult,
  DoctorServiceStatus,
  EvaluateResult,
  NormalizedResult,
} from '@/lib/result-types'

interface ReportRendererProps {
  result: NormalizedResult
  commandMode: string | null
}

export function ReportRenderer({ result, commandMode }: ReportRendererProps) {
  switch (result.type) {
    case 'ask':
      return <AskReport data={result.data} />
    case 'evaluate':
      return <EvaluateReport data={result.data} />
    case 'doctor':
      return <DoctorReport data={result.data} />
    case 'debug':
      return <DebugReport data={result.data} />
    default:
      // research / search raw text falls here — render as paragraphs
      if (result.type === 'raw' && (commandMode === 'research' || commandMode === 'search')) {
        return <RawTextReport data={result.data} />
      }
      return null
  }
}

// ---------------------------------------------------------------------------
// Shared components
// ---------------------------------------------------------------------------

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[#5f87af]">
      {children}
    </h3>
  )
}

function Collapsible({
  title,
  defaultOpen = false,
  children,
}: {
  title: string
  defaultOpen?: boolean
  children: React.ReactNode
}) {
  const [open, setOpen] = useState(defaultOpen)
  return (
    <div
      className="rounded-lg border border-[rgba(175,215,255,0.08)]"
      style={{ background: 'rgba(10, 18, 35, 0.3)' }}
    >
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left text-[11px] font-medium text-[#8787af] transition-colors hover:text-[#afd7ff]"
      >
        <span className="text-[10px]">{open ? '\u25BC' : '\u25B6'}</span>
        {title}
      </button>
      {open && (
        <div className="border-t border-[rgba(175,215,255,0.06)] px-3 py-2.5">{children}</div>
      )}
    </div>
  )
}

function TimingRow({ label, ms }: { label: string; ms: number }) {
  return (
    <div className="flex justify-between text-[12px]">
      <span className="text-[#8787af]">{label}</span>
      <span className="tabular-nums text-[#afd7ff]">{fmtMs(ms)}</span>
    </div>
  )
}

function fmtMs(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}

function fmtNum(n: number): string {
  return n.toLocaleString()
}

// ---------------------------------------------------------------------------
// Diagnostics panel (shared between ask and evaluate)
// ---------------------------------------------------------------------------

function DiagnosticsPanel({ diag }: { diag: AskDiagnostics }) {
  return (
    <div className="grid grid-cols-2 gap-x-6 gap-y-1 font-mono text-[12px]">
      <KV label="Candidate pool" value={fmtNum(diag.candidate_pool)} />
      <KV label="Reranked pool" value={fmtNum(diag.reranked_pool)} />
      <KV label="Chunks selected" value={fmtNum(diag.chunks_selected)} />
      <KV label="Full docs selected" value={fmtNum(diag.full_docs_selected)} />
      <KV label="Supplemental" value={fmtNum(diag.supplemental_selected)} />
      <KV label="Context chars" value={fmtNum(diag.context_chars)} />
      <KV label="Min relevance" value={diag.min_relevance_score.toFixed(3)} />
      <KV label="Doc fetch concurrency" value={String(diag.doc_fetch_concurrency)} />
    </div>
  )
}

function KV({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between">
      <span className="text-[#8787af]">{label}</span>
      <span className="text-[#afd7ff]">{value}</span>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Ask report
// ---------------------------------------------------------------------------

function AskReport({ data }: { data: AskResult }) {
  return (
    <div className="space-y-4">
      {/* Query */}
      <div>
        <SectionHeader>Query</SectionHeader>
        <p className="text-[13px] font-medium text-[#dce6f0]">{data.query}</p>
      </div>

      {/* Answer */}
      <div>
        <SectionHeader>Answer</SectionHeader>
        <div className="whitespace-pre-wrap text-[13px] leading-relaxed text-[#dce6f0]">
          {data.answer}
        </div>
      </div>

      {/* Timing */}
      {data.timing_ms && (
        <Collapsible title={`Timing (${fmtMs(data.timing_ms.total)})`} defaultOpen>
          <div className="space-y-1">
            <TimingRow label="Retrieval" ms={data.timing_ms.retrieval} />
            <TimingRow label="Context build" ms={data.timing_ms.context_build} />
            <TimingRow label="LLM" ms={data.timing_ms.llm} />
            <div className="mt-1 border-t border-[rgba(175,215,255,0.08)] pt-1">
              <TimingRow label="Total" ms={data.timing_ms.total} />
            </div>
          </div>
        </Collapsible>
      )}

      {/* Diagnostics */}
      {data.diagnostics && (
        <Collapsible title="Diagnostics">
          <DiagnosticsPanel diag={data.diagnostics} />
        </Collapsible>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Evaluate report
// ---------------------------------------------------------------------------

function EvaluateReport({ data }: { data: EvaluateResult }) {
  return (
    <div className="space-y-4">
      {/* Query */}
      <div>
        <SectionHeader>Query</SectionHeader>
        <p className="text-[13px] font-medium text-[#dce6f0]">{data.query}</p>
        <span className="mt-1 text-[11px] text-[#8787af]">
          {data.ref_chunk_count} reference chunks
        </span>
      </div>

      {/* Side-by-side RAG vs Baseline */}
      <div className="grid gap-3 md:grid-cols-2">
        <div>
          <SectionHeader>RAG Answer</SectionHeader>
          <div
            className="whitespace-pre-wrap rounded-lg border border-[rgba(135,215,135,0.15)] p-3 text-[12px] leading-relaxed text-[#dce6f0]"
            style={{ background: 'rgba(135, 215, 135, 0.04)' }}
          >
            {data.rag_answer}
          </div>
        </div>
        <div>
          <SectionHeader>Baseline Answer</SectionHeader>
          <div
            className="whitespace-pre-wrap rounded-lg border border-[rgba(255,175,135,0.15)] p-3 text-[12px] leading-relaxed text-[#dce6f0]"
            style={{ background: 'rgba(255, 175, 135, 0.04)' }}
          >
            {data.baseline_answer}
          </div>
        </div>
      </div>

      {/* Analysis */}
      <div>
        <SectionHeader>Analysis</SectionHeader>
        <div
          className="whitespace-pre-wrap rounded-lg border border-[rgba(135,175,255,0.15)] p-3 text-[12px] leading-relaxed text-[#dce6f0]"
          style={{ background: 'rgba(135, 175, 255, 0.04)' }}
        >
          {data.analysis_answer}
        </div>
      </div>

      {/* Timing */}
      {data.timing_ms && (
        <Collapsible title={`Timing (${fmtMs(data.timing_ms.total)})`} defaultOpen>
          <div className="space-y-1">
            <TimingRow label="Retrieval" ms={data.timing_ms.retrieval} />
            <TimingRow label="Context build" ms={data.timing_ms.context_build} />
            <TimingRow label="RAG LLM" ms={data.timing_ms.rag_llm} />
            <TimingRow label="Baseline LLM" ms={data.timing_ms.baseline_llm} />
            <TimingRow label="Research" ms={data.timing_ms.research_elapsed_ms} />
            <TimingRow label="Analysis LLM" ms={data.timing_ms.analysis_llm_ms} />
            <div className="mt-1 border-t border-[rgba(175,215,255,0.08)] pt-1">
              <TimingRow label="Total" ms={data.timing_ms.total} />
            </div>
          </div>
        </Collapsible>
      )}

      {/* Diagnostics */}
      {data.diagnostics && (
        <Collapsible title="Diagnostics">
          <DiagnosticsPanel diag={data.diagnostics} />
        </Collapsible>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Doctor report
// ---------------------------------------------------------------------------

function ServiceStatusIcon({ ok }: { ok: boolean }) {
  return ok ? (
    <span className="text-[#87d787]">{'\u2713'}</span>
  ) : (
    <span className="text-[#ff87af]">{'\u2717'}</span>
  )
}

function ServiceCard({ name, svc }: { name: string; svc: DoctorServiceStatus }) {
  return (
    <div
      className={`rounded-lg border p-2.5 ${svc.ok ? 'border-[rgba(135,215,135,0.15)]' : 'border-[rgba(255,135,175,0.2)]'}`}
      style={{
        background: svc.ok ? 'rgba(135, 215, 135, 0.04)' : 'rgba(255, 135, 175, 0.04)',
      }}
    >
      <div className="flex items-center gap-2">
        <ServiceStatusIcon ok={svc.ok} />
        <span className="text-[12px] font-medium text-[#dce6f0]">{name}</span>
      </div>
      {svc.url && <div className="mt-1 truncate text-[10px] text-[#5f6b7a]">{svc.url}</div>}
      {svc.model && <div className="mt-0.5 text-[10px] text-[#87afff]">{svc.model}</div>}
      {svc.detail && <div className="mt-0.5 text-[10px] text-[#8787af]">{svc.detail}</div>}
      {svc.summary && <div className="mt-0.5 text-[10px] text-[#8787af]">{svc.summary}</div>}
    </div>
  )
}

function DoctorReport({ data }: { data: DoctorResult }) {
  const serviceEntries = Object.entries(data.services)
  const pipelineEntries = Object.entries(data.pipelines ?? {})
  const queueEntries = Object.entries(data.queue_names ?? {})

  return (
    <div className="space-y-4">
      {/* Overall status */}
      <div className="flex items-center gap-2">
        <span
          className={`inline-block size-2.5 rounded-full ${data.all_ok ? 'bg-[#87d787] shadow-[0_0_6px_rgba(135,215,135,0.4)]' : 'bg-[#ff87af] shadow-[0_0_6px_rgba(255,135,175,0.4)]'}`}
        />
        <span className="text-[13px] font-medium text-[#dce6f0]">
          {data.all_ok ? 'All services healthy' : 'Some services need attention'}
        </span>
        {data.stale_jobs > 0 && (
          <span className="text-[11px] text-[#ffaf87]">{data.stale_jobs} stale jobs</span>
        )}
        {data.pending_jobs > 0 && (
          <span className="text-[11px] text-[#87afff]">{data.pending_jobs} pending</span>
        )}
      </div>

      {/* Service grid */}
      <div>
        <SectionHeader>Services</SectionHeader>
        <div className="grid grid-cols-2 gap-2 md:grid-cols-3">
          {serviceEntries.map(([name, svc]) => (
            <ServiceCard key={name} name={name} svc={svc} />
          ))}
        </div>
      </div>

      {/* Pipelines */}
      {pipelineEntries.length > 0 && (
        <Collapsible title="Pipelines" defaultOpen>
          <div className="space-y-1">
            {pipelineEntries.map(([name, ok]) => (
              <div key={name} className="flex items-center gap-2 text-[12px]">
                <ServiceStatusIcon ok={ok} />
                <span className="text-[#dce6f0]">{name}</span>
              </div>
            ))}
          </div>
        </Collapsible>
      )}

      {/* Queue names */}
      {queueEntries.length > 0 && (
        <Collapsible title="Queue Names">
          <div className="space-y-1 font-mono text-[12px]">
            {queueEntries.map(([key, val]) => (
              <div key={key} className="flex justify-between">
                <span className="text-[#8787af]">{key}</span>
                <span className="text-[#afd7ff]">{val}</span>
              </div>
            ))}
          </div>
        </Collapsible>
      )}

      {/* Browser runtime */}
      {data.browser_runtime && (
        <Collapsible title="Browser Runtime">
          <div className="font-mono text-[12px]">
            <KV label="Selection" value={data.browser_runtime.selection} />
          </div>
        </Collapsible>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Debug report (doctor + LLM analysis)
// ---------------------------------------------------------------------------

function DebugReport({ data }: { data: DebugResult }) {
  return (
    <div className="space-y-4">
      <DoctorReport data={data.doctor_report} />

      <div>
        <SectionHeader>LLM Debug Analysis</SectionHeader>
        <div className="mb-2 flex gap-4 text-[11px] text-[#8787af]">
          <span>
            Model: <span className="text-[#afd7ff]">{data.llm_debug.model}</span>
          </span>
          <span>
            Base URL: <span className="text-[#87afff]">{data.llm_debug.base_url}</span>
          </span>
        </div>
        <div
          className="whitespace-pre-wrap rounded-lg border border-[rgba(175,215,255,0.08)] p-3 text-[12px] leading-relaxed text-[#dce6f0]"
          style={{ background: 'rgba(10, 18, 35, 0.4)' }}
        >
          {data.llm_debug.analysis}
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Raw text report (research/search without --json)
// ---------------------------------------------------------------------------

function RawTextReport({ data }: { data: unknown[] }) {
  const text = data
    .map((item) => {
      if (typeof item === 'string') return item
      if (typeof item === 'object' && item !== null) return JSON.stringify(item, null, 2)
      return String(item)
    })
    .join('\n\n')

  return (
    <div className="whitespace-pre-wrap text-[13px] leading-relaxed text-[#dce6f0]">{text}</div>
  )
}
