'use client'

import { BrainCircuit, Clock3, Database, MessageSquareQuote, Search, Sparkles } from 'lucide-react'
import { useState } from 'react'
import { DoctorReport } from '@/components/results/doctor-report'
import { MarkdownBlock } from '@/components/results/markdown-block'
import { StructuredDataView } from '@/components/results/structured-data-view'
import type {
  AskDiagnostics,
  AskResult,
  DebugResult,
  EvaluateResult,
  NormalizedResult,
} from '@/lib/result-types'
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
    <h3 className="ui-label mb-1.5">
      {children}
    </h3>
  )
}

function AskPill({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  return (
    <span className="ui-chip-status rounded-md border border-[rgba(255,135,175,0.2)] bg-[rgba(14,25,48,0.62)] text-[var(--axon-accent-blue)]">
      {icon && <span className="text-[var(--axon-accent-blue-strong)]">{icon}</span>}
      <span className="uppercase tracking-wide text-[var(--axon-text-dim)]">{label}</span>
      <span className="text-[var(--axon-text-secondary)]">{value}</span>
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
      className="rounded-lg border border-[rgba(255,135,175,0.08)]"
      style={{ background: 'rgba(10, 18, 35, 0.3)' }}
    >
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[length:var(--text-sm)] font-medium text-[var(--axon-text-muted)] transition-colors hover:text-[var(--axon-accent-blue)]"
      >
        <span className="text-[length:var(--text-sm)]">{open ? '\u25BC' : '\u25B6'}</span>
        {title}
      </button>
      {open && <div className="border-t border-[rgba(255,135,175,0.06)] px-3 py-2">{children}</div>}
    </div>
  )
}

function TimingRow({ label, ms }: { label: string; ms: number }) {
  return (
    <div className="flex justify-between text-[length:var(--text-sm)]">
      <span className="text-[var(--axon-text-muted)]">{label}</span>
      <span className="tabular-nums text-[var(--axon-accent-blue)]">{fmtMs(ms)}</span>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Diagnostics panel (shared between ask and evaluate)
// ---------------------------------------------------------------------------

function DiagnosticsPanel({ diag }: { diag: AskDiagnostics }) {
  return (
    <div className="grid grid-cols-2 gap-x-6 gap-y-1 ui-mono">
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
      <span className="text-[var(--axon-text-muted)]">{label}</span>
      <span className="text-[var(--axon-accent-blue)]">{value}</span>
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
        className="rounded-lg border border-[rgba(255,135,175,0.14)] px-2 py-1"
        style={{
          background:
            'linear-gradient(130deg, rgba(14,24,46,0.78) 0%, rgba(10,18,36,0.58) 55%, rgba(24,16,38,0.5) 100%)',
        }}
      >
        <div className="flex flex-wrap items-center gap-1">
          <span className="inline-flex size-4 items-center justify-center rounded-[6px] border border-[rgba(175,215,255,0.3)] bg-[rgba(175,215,255,0.12)] text-[var(--axon-accent-pink-strong)]">
            <Sparkles size={10} />
          </span>
          <span className="ui-label">
            Ask
          </span>
          <p className="min-w-[180px] flex-1 break-words ui-long-copy">
            {data.query}
          </p>
          <AskPill label="total" value={fmtMs(data.timing_ms.total)} icon={<Clock3 size={10} />} />
          <AskPill
            label="ret"
            value={fmtMs(data.timing_ms.retrieval)}
            icon={<Search size={10} />}
          />
          <AskPill
            label="ctx"
            value={fmtMs(data.timing_ms.context_build)}
            icon={<Database size={10} />}
          />
          <AskPill
            label="llm"
            value={fmtMs(data.timing_ms.llm)}
            icon={<BrainCircuit size={10} />}
          />
          {diag && <AskPill label="chunks" value={fmtNum(diag.chunks_selected)} />}
          {diag && <AskPill label="docs" value={fmtNum(diag.full_docs_selected)} />}
          {diag && <AskPill label="chars" value={fmtNum(diag.context_chars)} />}
        </div>
      </div>

      {/* Answer */}
      <div
        className="rounded-lg border border-[rgba(255,135,175,0.12)] px-2 py-1.5"
        style={{
          background:
            'linear-gradient(145deg, rgba(11,20,40,0.72), rgba(8,15,31,0.52) 60%, rgba(20,14,36,0.42))',
        }}
      >
        <div className="ui-label mb-1 flex items-center gap-1">
          <MessageSquareQuote size={11} className="text-[var(--axon-accent-blue)]" />
          <span>Answer</span>
        </div>
        <div className="animate-in fade-in-0 duration-500">
          <MarkdownBlock markdown={data.answer} className="ui-long-copy" />
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
        <p className="text-[length:var(--text-base)] font-medium text-[var(--axon-text-secondary)]">{data.query}</p>
        <span className="ui-meta mt-1">
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
            <div className="mt-1 border-t border-[rgba(255,135,175,0.08)] pt-1">
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
        <div className="ui-meta mb-2 flex gap-4">
          <span>
            Model: <span className="text-[var(--axon-accent-blue)]">{data.llm_debug.model}</span>
          </span>
          <span>
            Base URL:{' '}
            <span className="text-[var(--axon-accent-blue-strong)]">{data.llm_debug.base_url}</span>
          </span>
        </div>
        <div
          className="whitespace-pre-wrap rounded-lg border border-[rgba(255,135,175,0.08)] p-3 ui-long-copy"
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

  return (
    <div className="space-y-3">
      {data.map((item, idx) => {
        if (typeof item === 'object' && item !== null) {
          return <StructuredDataView key={`object-${idx}`} data={item} />
        }
        const text = typeof item === 'string' ? item : formatStructuredText(item)
        return (
          <div
            key={`text-${idx}`}
            className="whitespace-pre-wrap ui-long-copy"
          >
            {text}
          </div>
        )
      })}
    </div>
  )
}
