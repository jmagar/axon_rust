'use client'

import { useState } from 'react'
import { BrainCircuit, Clock3, Database, MessageSquareQuote, Search, Sparkles } from 'lucide-react'
import { DoctorReport } from '@/components/results/doctor-report'
import { MarkdownBlock } from '@/components/results/markdown-block'
import type {
  AskDiagnostics,
  AskResult,
  DebugResult,
  EvaluateResult,
  NormalizedResult,
} from '@/lib/result-types'
import { StructuredDataView } from '@/components/results/structured-data-view'
import { formatStructuredText } from '@/lib/structured-text'
import { fmtMs, fmtNum } from './shared'

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
    <h3 className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-[#5f87af]">
      {children}
    </h3>
  )
}

function IconSectionHeader({
  icon,
  children,
  glow = 'blue',
}: {
  icon: React.ReactNode
  children: React.ReactNode
  glow?: 'blue' | 'pink'
}) {
  return (
    <div className="mb-1.5 flex items-center gap-2">
      <span
        className={`inline-flex size-4 items-center justify-center rounded-[6px] border ${
          glow === 'pink'
            ? 'border-[rgba(255,135,175,0.3)] bg-[rgba(255,135,175,0.12)] text-[#ff9ec0]'
            : 'border-[rgba(175,215,255,0.26)] bg-[rgba(175,215,255,0.12)] text-[#95cbff]'
        }`}
      >
        {icon}
      </span>
      <h3 className="text-[10px] font-semibold uppercase tracking-wider text-[#7ea7d8]">
        {children}
      </h3>
    </div>
  )
}

function AskPill({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-md border border-[rgba(175,215,255,0.2)] bg-[rgba(14,25,48,0.62)] px-2 py-0.5 font-mono text-[8px] text-[#b8d6ff]">
      {icon && <span className="text-[#89c0ff]">{icon}</span>}
      <span className="uppercase tracking-wide text-[#7ba0c7]">{label}</span>
      <span className="text-[#dce6f0]">{value}</span>
    </span>
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
        className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[10px] font-medium text-[#8787af] transition-colors hover:text-[#afd7ff]"
      >
        <span className="text-[10px]">{open ? '\u25BC' : '\u25B6'}</span>
        {title}
      </button>
      {open && <div className="border-t border-[rgba(175,215,255,0.06)] px-3 py-2">{children}</div>}
    </div>
  )
}

function TimingRow({ label, ms }: { label: string; ms: number }) {
  return (
    <div className="flex justify-between text-[10px]">
      <span className="text-[#8787af]">{label}</span>
      <span className="tabular-nums text-[#afd7ff]">{fmtMs(ms)}</span>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Diagnostics panel (shared between ask and evaluate)
// ---------------------------------------------------------------------------

function DiagnosticsPanel({ diag }: { diag: AskDiagnostics }) {
  return (
    <div className="grid grid-cols-2 gap-x-6 gap-y-1 font-mono text-[10px]">
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
  const diag = data.diagnostics

  return (
    <div className="space-y-2 animate-in fade-in-0 slide-in-from-bottom-1 duration-300">
      {/* Omnibox-style meta strip */}
      <div
        className="rounded-lg border border-[rgba(175,215,255,0.14)] px-2 py-1"
        style={{
          background:
            'linear-gradient(130deg, rgba(14,24,46,0.78) 0%, rgba(10,18,36,0.58) 55%, rgba(24,16,38,0.5) 100%)',
        }}
      >
        <div className="flex flex-wrap items-center gap-1">
          <span className="inline-flex size-4 items-center justify-center rounded-[6px] border border-[rgba(255,135,175,0.3)] bg-[rgba(255,135,175,0.12)] text-[#ff9ec0]">
            <Sparkles size={10} />
          </span>
          <span className="text-[9px] uppercase tracking-wider text-[#7ea7d8]">Ask</span>
          <p className="min-w-[180px] flex-1 break-words text-[9px] leading-[1.3] text-[#dce6f0]">
            {data.query}
          </p>
          <AskPill label="total" value={fmtMs(data.timing_ms.total)} icon={<Clock3 size={9} />} />
          <AskPill label="ret" value={fmtMs(data.timing_ms.retrieval)} icon={<Search size={9} />} />
          <AskPill label="ctx" value={fmtMs(data.timing_ms.context_build)} icon={<Database size={9} />} />
          <AskPill label="llm" value={fmtMs(data.timing_ms.llm)} icon={<BrainCircuit size={9} />} />
          {diag && <AskPill label="chunks" value={fmtNum(diag.chunks_selected)} />}
          {diag && <AskPill label="docs" value={fmtNum(diag.full_docs_selected)} />}
          {diag && <AskPill label="chars" value={fmtNum(diag.context_chars)} />}
        </div>
      </div>

      {/* Answer */}
      <div
        className="rounded-lg border border-[rgba(175,215,255,0.12)] px-2 py-1.5"
        style={{
          background:
            'linear-gradient(145deg, rgba(11,20,40,0.72), rgba(8,15,31,0.52) 60%, rgba(20,14,36,0.42))',
        }}
      >
        <div className="mb-1 flex items-center gap-1 text-[9px] uppercase tracking-wider text-[#7ea7d8]">
          <MessageSquareQuote size={10} className="text-[#95cbff]" />
          <span>Answer</span>
        </div>
        <div className="animate-in fade-in-0 duration-500">
          <MarkdownBlock markdown={data.answer} className="text-[9px] leading-[1.36]" />
        </div>
      </div>
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
            className="rounded-lg border border-[rgba(135,215,135,0.15)] p-2.5"
            style={{ background: 'rgba(135, 215, 135, 0.04)' }}
          >
            <MarkdownBlock markdown={data.rag_answer} />
          </div>
        </div>
        <div>
          <SectionHeader>Baseline Answer</SectionHeader>
          <div
            className="rounded-lg border border-[rgba(255,175,135,0.15)] p-2.5"
            style={{ background: 'rgba(255, 175, 135, 0.04)' }}
          >
            <MarkdownBlock markdown={data.baseline_answer} />
          </div>
        </div>
      </div>

      {/* Analysis */}
      <div>
        <SectionHeader>Analysis</SectionHeader>
        <div
          className="rounded-lg border border-[rgba(135,175,255,0.15)] p-2.5"
          style={{ background: 'rgba(135, 175, 255, 0.04)' }}
        >
          <MarkdownBlock markdown={data.analysis_answer} />
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
  const objectItems = data.filter((item) => typeof item === 'object' && item !== null)
  const stringItems = data.filter((item) => typeof item === 'string').map((item) => item as string)

  if (objectItems.length > 0 && stringItems.length === 0) {
    if (objectItems.length === 1) {
      return <StructuredDataView data={objectItems[0]} />
    }
    return (
      <div className="space-y-3">
        {objectItems.map((item, idx) => (
          <StructuredDataView key={idx} data={item} />
        ))}
      </div>
    )
  }

  const text = data
    .map((item) => {
      if (typeof item === 'string') return item
      if (typeof item === 'object' && item !== null) return formatStructuredText(item)
      return String(item)
    })
    .join('\n\n')

  return (
    <div className="whitespace-pre-wrap text-[13px] leading-relaxed text-[#dce6f0]">{text}</div>
  )
}
