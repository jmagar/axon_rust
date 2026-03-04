'use client'

import { Activity, Clock3, Hash, Sparkles, WandSparkles, Zap } from 'lucide-react'
import { type FormEvent, useEffect, useMemo, useRef, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { apiFetch } from '@/lib/api-fetch'
import type { WsServerMsg } from '@/lib/ws-protocol'

type EvaluateTokenStream = 'with_context' | 'without_context'

interface EvaluateStreamDoneEvent {
  type: 'stream_done'
  stream: EvaluateTokenStream
  elapsed_ms?: number
  chars?: number
}

interface EvaluateTokenEvent {
  type: 'token'
  stream: EvaluateTokenStream
  delta: string
}

interface EvaluateCompleteEvent {
  type: 'evaluate_complete'
  query: string
  rag_answer: string
  baseline_answer: string
  analysis_answer: string
  source_urls?: string[]
  timing_ms?: {
    retrieval?: number
    context_build?: number
    rag_llm?: number
    baseline_llm?: number
    research_elapsed_ms?: number
    analysis_llm_ms?: number
    total?: number
  }
}

interface SuggestApiResult {
  suggestions?: Array<{ url?: string; reason?: string }>
}

interface EvaluateStartEvent {
  type: 'evaluate_start'
  query: string
  stage?: string
  context?: {
    source_count?: number
    source_urls?: string[]
  }
}

type EvaluateEvent =
  | EvaluateTokenEvent
  | EvaluateStreamDoneEvent
  | EvaluateCompleteEvent
  | EvaluateStartEvent
  | { type: 'analysis_start' }
  | Record<string, unknown>

type DetailTab = 'event' | 'analysis' | 'suggest' | 'timing'

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null
}

function formatMs(value?: number): string {
  if (typeof value !== 'number' || Number.isNaN(value)) return '-'
  return `${Math.round(value)}ms`
}

function estimateTokens(chars?: number): number | null {
  if (typeof chars !== 'number' || Number.isNaN(chars) || chars <= 0) return null
  return Math.max(1, Math.round(chars / 4))
}

function prettifySlug(slug: string): string {
  const cleaned = slug
    .replace(/\.[a-z0-9]+$/i, '')
    .replace(/[-_]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
  if (!cleaned) return 'Untitled Document'
  return cleaned.replace(/\b\w/g, (ch) => ch.toUpperCase())
}

function sourceDocLabel(url: string): string {
  try {
    const parsed = new URL(url)
    const segments = parsed.pathname.split('/').filter(Boolean)
    if (segments.length === 0) return parsed.hostname
    return prettifySlug(segments[segments.length - 1] ?? parsed.hostname)
  } catch {
    return prettifySlug(url)
  }
}

function commandExecId(msg: WsServerMsg): string | null {
  if (
    msg.type === 'command.start' ||
    msg.type === 'command.output.json' ||
    msg.type === 'command.output.line' ||
    msg.type === 'command.done' ||
    msg.type === 'command.error'
  ) {
    return msg.data.ctx.exec_id
  }
  return null
}

export default function EvaluatePage() {
  const { send, subscribe, status } = useAxonWs()
  const [query, setQuery] = useState('')
  const [running, setRunning] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [events, setEvents] = useState<EvaluateEvent[]>([])
  const [withContext, setWithContext] = useState('')
  const [withoutContext, setWithoutContext] = useState('')
  const [analysis, setAnalysis] = useState('')
  const [sourceDocs, setSourceDocs] = useState<string[]>([])
  const [phaseLabel, setPhaseLabel] = useState<string>('idle')
  const [finalEvent, setFinalEvent] = useState<EvaluateCompleteEvent | null>(null)
  const [commandElapsedMs, setCommandElapsedMs] = useState<number | null>(null)
  const [suggestions, setSuggestions] = useState<Array<{ url: string; reason: string }>>([])
  const [suggestLoading, setSuggestLoading] = useState(false)
  const [suggestError, setSuggestError] = useState<string | null>(null)
  const [activeTab, setActiveTab] = useState<DetailTab>('analysis')
  const [streamTimings, setStreamTimings] = useState<
    Partial<Record<EvaluateTokenStream, { elapsedMs?: number; chars?: number }>>
  >({})

  const activeExecIdRef = useRef<string | null>(null)
  const unsubscribeRef = useRef<(() => void) | null>(null)

  useEffect(() => {
    return () => {
      unsubscribeRef.current?.()
      unsubscribeRef.current = null
    }
  }, [])

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const trimmed = query.trim()
    if (!trimmed || running) return

    unsubscribeRef.current?.()
    unsubscribeRef.current = null
    activeExecIdRef.current = null
    setRunning(true)
    setError(null)
    setEvents([])
    setWithContext('')
    setWithoutContext('')
    setAnalysis('')
    setFinalEvent(null)
    setCommandElapsedMs(null)
    setStreamTimings({})
    setSourceDocs([])
    setPhaseLabel('starting')
    setSuggestions([])
    setSuggestLoading(false)
    setSuggestError(null)

    const unsubscribe = subscribe((msg) => {
      if (msg.type === 'command.start') {
        if (msg.data.ctx.mode !== 'evaluate') return
        activeExecIdRef.current = msg.data.ctx.exec_id
        return
      }

      const expectedExecId = activeExecIdRef.current
      const msgExecId = commandExecId(msg)
      const isFallbackTerminal =
        !expectedExecId && (msg.type === 'command.error' || msg.type === 'command.done')
      if (!isFallbackTerminal) {
        if (!expectedExecId || msgExecId !== expectedExecId) return
      }

      if (msg.type === 'command.output.json') {
        const evt = asRecord(msg.data.data)
        if (!evt) return
        setEvents((prev) => [...prev.slice(-199), evt])

        if (evt.type === 'token') {
          const token = evt as unknown as EvaluateTokenEvent
          if (token.stream === 'with_context') {
            setWithContext((prev) => prev + token.delta)
          } else if (token.stream === 'without_context') {
            setWithoutContext((prev) => prev + token.delta)
          }
          return
        }

        if (evt.type === 'evaluate_start') {
          const start = evt as unknown as EvaluateStartEvent
          setPhaseLabel(start.stage ?? 'running')
          if (Array.isArray(start.context?.source_urls)) {
            setSourceDocs(start.context.source_urls.filter((item) => typeof item === 'string'))
          }
          return
        }

        if (evt.type === 'evaluate_context_ready') {
          const ready = evt as unknown as EvaluateStartEvent
          setPhaseLabel('streaming_answers')
          if (Array.isArray(ready.context?.source_urls)) {
            setSourceDocs(ready.context.source_urls.filter((item) => typeof item === 'string'))
          }
          return
        }

        if (evt.type === 'stream_done') {
          const done = evt as unknown as EvaluateStreamDoneEvent
          setStreamTimings((prev) => ({
            ...prev,
            [done.stream]: { elapsedMs: done.elapsed_ms, chars: done.chars },
          }))
          setPhaseLabel('analyzing')
          return
        }

        if (evt.type === 'evaluate_complete') {
          const complete = evt as unknown as EvaluateCompleteEvent
          setFinalEvent(complete)
          setWithContext(complete.rag_answer)
          setWithoutContext(complete.baseline_answer)
          setAnalysis(complete.analysis_answer)
          if (Array.isArray(complete.source_urls)) {
            setSourceDocs(complete.source_urls.filter((item) => typeof item === 'string'))
          }
          setPhaseLabel('complete')
          setSuggestLoading(true)
          setSuggestError(null)
          const suggestHeaders: HeadersInit = {}
          const apiToken = process.env.NEXT_PUBLIC_AXON_API_TOKEN
          if (apiToken) suggestHeaders['x-api-key'] = apiToken
          void apiFetch(`/api/cortex/suggest?q=${encodeURIComponent(complete.query)}`, {
            headers: suggestHeaders,
          })
            .then((res) => {
              if (!res.ok) throw new Error(`suggest request failed (${res.status})`)
              return res.json() as Promise<{ ok?: boolean; data?: SuggestApiResult }>
            })
            .then((payload) => {
              const raw = payload?.data?.suggestions ?? []
              const next = raw
                .filter(
                  (item): item is { url: string; reason: string } =>
                    typeof item?.url === 'string' &&
                    item.url.length > 0 &&
                    typeof item?.reason === 'string',
                )
                .slice(0, 12)
              setSuggestions(next)
            })
            .catch((err: unknown) => {
              const message = err instanceof Error ? err.message : 'Failed to load suggestions'
              setSuggestError(message)
            })
            .finally(() => {
              setSuggestLoading(false)
            })
        }
        return
      }

      if (msg.type === 'command.error') {
        setError(msg.data.payload.message)
        setRunning(false)
        setCommandElapsedMs(msg.data.payload.elapsed_ms ?? null)
        unsubscribeRef.current?.()
        unsubscribeRef.current = null
        setPhaseLabel('error')
        return
      }

      if (msg.type === 'command.done') {
        setRunning(false)
        setCommandElapsedMs(msg.data.payload.elapsed_ms ?? null)
        unsubscribeRef.current?.()
        unsubscribeRef.current = null
        setPhaseLabel((cur) => (cur === 'complete' ? cur : 'done'))
      }
    })

    unsubscribeRef.current = unsubscribe
    send({
      type: 'execute',
      mode: 'evaluate',
      input: trimmed,
      flags: { responses_mode: 'events', diagnostics: true },
    })
  }

  const jsonLines = useMemo(() => events.map((evt) => JSON.stringify(evt)), [events])

  const ragTiming = streamTimings.with_context
  const baselineTiming = streamTimings.without_context
  const ragTokens = estimateTokens(ragTiming?.chars)
  const baselineTokens = estimateTokens(baselineTiming?.chars)
  const analysisTokens = estimateTokens(analysis.length)
  const totalTokens =
    (ragTokens ?? 0) + (baselineTokens ?? 0) + (analysisTokens ?? 0) > 0
      ? (ragTokens ?? 0) + (baselineTokens ?? 0) + (analysisTokens ?? 0)
      : null

  return (
    <main className="mx-auto flex h-[calc(100vh-2.8rem)] w-full max-w-[1700px] flex-col gap-0.5 overflow-hidden px-2 py-1 sm:px-3">
      <div className="flex shrink-0 items-center gap-1.5 px-0.5 py-0.5">
        <h1 className="text-[11px] font-semibold text-[var(--text-primary)]">Evaluate</h1>
        <span
          className={`inline-block size-1.5 rounded-full ${
            status === 'connected'
              ? 'bg-emerald-400'
              : status === 'reconnecting'
                ? 'bg-amber-400'
                : 'bg-rose-400'
          }`}
        />
        <span className="text-[10px] text-[var(--text-dim)]">{running ? phaseLabel : 'idle'}</span>
      </div>

      <section className="grid min-h-0 flex-1 grid-rows-[minmax(0,2.5fr)_minmax(0,1fr)] gap-0.5">
        <div className="grid min-h-0 gap-0.5 xl:grid-cols-[140px_1fr_1fr]">
          <aside className="min-h-0 flex flex-col rounded-xl border border-[var(--border-subtle)] bg-[rgba(9,16,30,0.68)] p-1">
            <h2 className="mb-0.5 whitespace-nowrap text-[11px] font-semibold text-[var(--text-primary)]">
              Context Docs
            </h2>
            <p className="mb-0.5 text-[10px] text-[var(--text-dim)]">
              Sources: {sourceDocs.length}
            </p>
            {sourceDocs.length > 0 ? (
              <ul className="min-h-0 flex-1 space-y-1 overflow-auto">
                {sourceDocs.map((url) => (
                  <li key={url} className="rounded border border-[var(--border-subtle)] p-1.5">
                    <a
                      href={url}
                      target="_blank"
                      rel="noreferrer"
                      className="block text-[11px] text-[var(--axon-primary)] hover:underline"
                      title={url}
                    >
                      {sourceDocLabel(url)}
                    </a>
                    <p className="truncate text-[10px] text-[var(--text-dim)]">
                      {(() => {
                        try {
                          return new URL(url).hostname
                        } catch {
                          return url
                        }
                      })()}
                    </p>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-[11px] text-[var(--text-dim)]">
                No source documents captured yet.
              </p>
            )}
          </aside>

          <article className="min-h-0 flex flex-col rounded-xl border border-[rgba(135,215,135,0.24)] bg-[rgba(135,215,135,0.06)] p-1">
            <h2 className="mb-0.5 whitespace-nowrap text-[11px] font-semibold text-[var(--text-primary)]">
              With Context
            </h2>
            <div className="mb-0.5 flex items-center gap-2 text-[10px]">
              <span className="flex items-center gap-0.5 text-amber-400/80">
                <Clock3 className="size-2.5" />
                {formatMs(ragTiming?.elapsedMs)}
              </span>
              <span className="flex items-center gap-0.5 text-sky-400/80">
                <Hash className="size-2.5" />
                {typeof ragTiming?.chars === 'number' ? ragTiming.chars : '-'}
              </span>
              <span className="flex items-center gap-0.5 text-violet-400/80">
                <Zap className="size-2.5" />~{ragTokens ?? '-'}
              </span>
            </div>
            <pre className="flex-1 min-h-0 overflow-auto whitespace-pre-wrap text-[11px] leading-[1.28] text-[var(--text-secondary)]">
              {withContext || 'Awaiting stream...'}
            </pre>
          </article>

          <article className="min-h-0 flex flex-col rounded-xl border border-[rgba(255,175,135,0.24)] bg-[rgba(255,175,135,0.06)] p-1">
            <h2 className="mb-0.5 whitespace-nowrap text-[11px] font-semibold text-[var(--text-primary)]">
              Without Context
            </h2>
            <div className="mb-0.5 flex items-center gap-2 text-[10px]">
              <span className="flex items-center gap-0.5 text-amber-400/80">
                <Clock3 className="size-2.5" />
                {formatMs(baselineTiming?.elapsedMs)}
              </span>
              <span className="flex items-center gap-0.5 text-sky-400/80">
                <Hash className="size-2.5" />
                {typeof baselineTiming?.chars === 'number' ? baselineTiming.chars : '-'}
              </span>
              <span className="flex items-center gap-0.5 text-violet-400/80">
                <Zap className="size-2.5" />~{baselineTokens ?? '-'}
              </span>
            </div>
            <pre className="flex-1 min-h-0 overflow-auto whitespace-pre-wrap text-[11px] leading-[1.28] text-[var(--text-secondary)]">
              {withoutContext || 'Awaiting stream...'}
            </pre>
          </article>
        </div>

        <section className="min-h-0 flex flex-col rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-float)] p-1">
          <div className="mb-0.5 flex shrink-0 flex-wrap gap-0.5">
            {(
              [
                ['analysis', 'LLM Judge Analysis', WandSparkles],
                ['event', 'Event Stream', Activity],
                ['suggest', 'Suggest', Sparkles],
                ['timing', 'Timings', Clock3],
              ] as Array<[DetailTab, string, typeof Activity]>
            ).map(([id, label, Icon]) => (
              <button
                key={id}
                type="button"
                onClick={() => setActiveTab(id)}
                aria-label={label}
                title={label}
                className={`rounded border px-2 py-1 text-[10px] ${
                  activeTab === id
                    ? 'border-[rgba(135,175,255,0.5)] bg-[rgba(135,175,255,0.15)] text-[var(--axon-primary)]'
                    : 'border-[var(--border-subtle)] text-[var(--text-dim)]'
                }`}
              >
                <Icon className="size-3.5" />
              </button>
            ))}
          </div>

          <div className="flex-1 min-h-0 overflow-auto">
            {activeTab === 'event' && (
              <pre className="h-full overflow-auto whitespace-pre-wrap text-[10px] leading-3.5 text-[var(--text-secondary)]">
                {jsonLines.length > 0 ? jsonLines.join('\n') : 'No events yet.'}
              </pre>
            )}

            {activeTab === 'analysis' && (
              <pre className="h-full overflow-auto whitespace-pre-wrap text-[11px] leading-4 text-[var(--text-secondary)]">
                {analysis || 'Analysis will appear after both streams finish.'}
              </pre>
            )}

            {activeTab === 'suggest' &&
              (suggestLoading ? (
                <p className="text-[11px] text-[var(--text-dim)]">Loading suggestions...</p>
              ) : suggestError ? (
                <p className="text-[11px] text-[#fca5a5]">{suggestError}</p>
              ) : suggestions.length > 0 ? (
                <div className="h-full space-y-1 overflow-auto">
                  {suggestions.map((item) => (
                    <div
                      key={item.url}
                      className="rounded border border-[var(--border-subtle)] p-1.5"
                    >
                      <p className="break-all text-[11px] font-medium text-[var(--text-primary)]">
                        {item.url}
                      </p>
                      <p className="mt-0.5 text-[10px] text-[var(--text-dim)]">{item.reason}</p>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-[11px] text-[var(--text-dim)]">
                  Run evaluate to generate suggest-command crawl recommendations.
                </p>
              ))}

            {activeTab === 'timing' && (
              <div className="grid grid-cols-2 gap-x-3 gap-y-0.5 text-[11px] text-[var(--text-secondary)]">
                <span>retrieval</span>
                <span>{formatMs(finalEvent?.timing_ms?.retrieval)}</span>
                <span>context_build</span>
                <span>{formatMs(finalEvent?.timing_ms?.context_build)}</span>
                <span>rag_llm</span>
                <span>{formatMs(finalEvent?.timing_ms?.rag_llm)}</span>
                <span>baseline_llm</span>
                <span>{formatMs(finalEvent?.timing_ms?.baseline_llm)}</span>
                <span>research_elapsed</span>
                <span>{formatMs(finalEvent?.timing_ms?.research_elapsed_ms)}</span>
                <span>analysis_llm</span>
                <span>{formatMs(finalEvent?.timing_ms?.analysis_llm_ms)}</span>
                <span>total (evaluate)</span>
                <span>{formatMs(finalEvent?.timing_ms?.total)}</span>
                <span>total (command)</span>
                <span>{formatMs(commandElapsedMs ?? undefined)}</span>
                <span>tokens~ (RAG)</span>
                <span>{ragTokens ?? '-'}</span>
                <span>tokens~ (baseline)</span>
                <span>{baselineTokens ?? '-'}</span>
                <span>tokens~ (analysis)</span>
                <span>{analysisTokens ?? '-'}</span>
                <span>tokens~ (total)</span>
                <span>{totalTokens ?? '-'}</span>
              </div>
            )}
          </div>
        </section>
      </section>

      <section className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-float)] p-1">
        <form className="flex flex-col gap-1.5 sm:flex-row sm:items-center" onSubmit={handleSubmit}>
          <input
            value={query}
            onChange={(evt) => setQuery(evt.target.value)}
            placeholder="Ask a question to evaluate RAG vs baseline..."
            className="min-w-0 flex-1 rounded-md border border-[var(--border-subtle)] bg-[rgba(8,14,27,0.55)] px-2 py-1 text-xs text-[var(--text-primary)] outline-none focus:border-[var(--axon-primary)]"
          />
          <button
            type="submit"
            disabled={running || query.trim().length === 0}
            className="rounded-md border border-[rgba(135,175,255,0.4)] bg-[rgba(135,175,255,0.12)] px-3 py-1 text-xs font-medium text-[var(--axon-primary)] disabled:cursor-not-allowed disabled:opacity-40"
          >
            {running ? 'Running...' : 'Evaluate'}
          </button>
        </form>
        {error && <p className="mt-1.5 text-xs text-[#fca5a5]">{error}</p>}
      </section>
    </main>
  )
}
