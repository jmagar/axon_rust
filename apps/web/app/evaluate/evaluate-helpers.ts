import type { WsServerMsg } from '@/lib/ws-protocol'

export type EvaluateTokenStream = 'with_context' | 'without_context'

export interface EvaluateStreamDoneEvent {
  type: 'stream_done'
  stream: EvaluateTokenStream
  elapsed_ms?: number
  chars?: number
}

export interface EvaluateTokenEvent {
  type: 'token'
  stream: EvaluateTokenStream
  delta: string
}

export interface EvaluateCompleteEvent {
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

export interface SuggestApiResult {
  suggestions?: Array<{ url?: string; reason?: string }>
}

export interface EvaluateStartEvent {
  type: 'evaluate_start'
  query: string
  stage?: string
  context?: {
    source_count?: number
    source_urls?: string[]
  }
}

export type EvaluateEvent =
  | EvaluateTokenEvent
  | EvaluateStreamDoneEvent
  | EvaluateCompleteEvent
  | EvaluateStartEvent
  | { type: 'analysis_start' }
  | Record<string, unknown>

export type DetailTab = 'event' | 'analysis' | 'suggest' | 'timing'

export function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null
}

export function formatMs(value?: number): string {
  if (typeof value !== 'number' || Number.isNaN(value)) return '-'
  return `${Math.round(value)}ms`
}

export function estimateTokens(chars?: number): number | null {
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

export function sourceDocLabel(url: string): string {
  try {
    const parsed = new URL(url)
    const segments = parsed.pathname.split('/').filter(Boolean)
    if (segments.length === 0) return parsed.hostname
    return prettifySlug(segments[segments.length - 1] ?? parsed.hostname)
  } catch {
    return prettifySlug(url)
  }
}

export function commandExecId(msg: WsServerMsg): string | null {
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
