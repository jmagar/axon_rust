/**
 * TypeScript interfaces for each Axon CLI command's --json output.
 *
 * These shapes mirror the Rust structs serialized via serde in:
 * - crates/cli/commands/*.rs (per-command JSON output)
 * - crates/vector/ops/commands/*.rs (query/ask/evaluate)
 * - crates/core/health.rs (doctor)
 */

// ---------------------------------------------------------------------------
// query: one object per stdout line (array of these)
// ---------------------------------------------------------------------------
export interface QueryResult {
  rank: number
  score: number
  rerank_score?: number
  url: string
  source: string
  snippet: string
}

// ---------------------------------------------------------------------------
// ask: single object
// ---------------------------------------------------------------------------
export interface AskDiagnostics {
  candidate_pool: number
  reranked_pool: number
  chunks_selected: number
  full_docs_selected: number
  supplemental_selected: number
  context_chars: number
  min_relevance_score: number
  doc_fetch_concurrency: number
}

export interface AskResult {
  query: string
  answer: string
  diagnostics?: AskDiagnostics
  timing_ms: {
    retrieval: number
    context_build: number
    llm: number
    total: number
  }
}

// ---------------------------------------------------------------------------
// evaluate: single object
// ---------------------------------------------------------------------------
export interface EvaluateResult {
  query: string
  rag_answer: string
  baseline_answer: string
  analysis_answer: string
  ref_chunk_count: number
  diagnostics?: AskDiagnostics
  timing_ms: {
    retrieval: number
    context_build: number
    rag_llm: number
    baseline_llm: number
    research_elapsed_ms: number
    analysis_llm_ms: number
    total: number
  }
}

// ---------------------------------------------------------------------------
// doctor: single object
// ---------------------------------------------------------------------------
export interface DoctorServiceStatus {
  ok: boolean
  url?: string
  configured?: boolean
  detail?: string
  model?: string
  summary?: string
  latency_ms?: number
  info_latency_ms?: number
}

export interface DoctorResult {
  observed_at_utc?: string
  services: Record<string, DoctorServiceStatus>
  pipelines: Record<string, boolean>
  queue_names: Record<string, string>
  browser_runtime?: { selection: string; diagnostics?: unknown }
  timing_ms?: {
    crawl_report?: number
    extract_report?: number
    embed_report?: number
    ingest_report?: number
    stale_pending?: number
  }
  stale_jobs: number
  pending_jobs: number
  all_ok: boolean
}

// ---------------------------------------------------------------------------
// debug: single object
// ---------------------------------------------------------------------------
export interface DebugResult {
  doctor_report: DoctorResult
  llm_debug: {
    model: string
    base_url: string
    analysis: string
  }
}

// ---------------------------------------------------------------------------
// map: single object
// ---------------------------------------------------------------------------
export interface MapResult {
  url: string
  mapped_urls: number
  sitemap_urls: number
  pages_seen: number
  thin_pages: number
  elapsed_ms: number
  urls: string[]
}

// ---------------------------------------------------------------------------
// sources: key-value map {url: count}
// ---------------------------------------------------------------------------
export type SourcesResult = Record<string, number>

// ---------------------------------------------------------------------------
// domains: key-value map {domain: count} or {domain: [url_count, vector_count]}
// ---------------------------------------------------------------------------
export type DomainsResult = Record<string, number | [number, number]>

// ---------------------------------------------------------------------------
// stats: single object
// ---------------------------------------------------------------------------
export interface StatsResult {
  collection: string
  status: string
  indexed_vectors_count: number
  points_count: number
  dimension: number
  distance: string
  segments_count: number
  docs_embedded_estimate: number
  avg_chunks_per_doc: number
  payload_fields: string[]
  counts: Record<string, number>
  [key: string]: unknown
}

// ---------------------------------------------------------------------------
// status: single object with job arrays
// ---------------------------------------------------------------------------
export interface JobEntry {
  id: string
  url?: string
  status: string
  [key: string]: unknown
}

export interface StatusResult {
  local_crawl_jobs: JobEntry[]
  local_extract_jobs: JobEntry[]
  local_embed_jobs: JobEntry[]
  local_ingest_jobs: JobEntry[]
}

// ---------------------------------------------------------------------------
// suggest: single object
// ---------------------------------------------------------------------------
export interface SuggestResult {
  collection: string
  requested: number
  indexed_urls_count: number
  suggestions: Array<{ url: string; reason: string }>
  rejected_existing: string[]
}

// ---------------------------------------------------------------------------
// retrieve: single object
// ---------------------------------------------------------------------------
export interface RetrieveResult {
  url: string
  chunks: number
  content: string
}

// ---------------------------------------------------------------------------
// dedupe: single object
// ---------------------------------------------------------------------------
export interface DedupeResult {
  duplicate_groups: number
  deleted: number
  collection: string
}

// ---------------------------------------------------------------------------
// Discriminated union for normalized results
// ---------------------------------------------------------------------------
export type NormalizedResult =
  | { type: 'query'; data: QueryResult[] }
  | { type: 'ask'; data: AskResult }
  | { type: 'evaluate'; data: EvaluateResult }
  | { type: 'doctor'; data: DoctorResult }
  | { type: 'debug'; data: DebugResult }
  | { type: 'map'; data: MapResult }
  | { type: 'sources'; data: SourcesResult }
  | { type: 'domains'; data: DomainsResult }
  | { type: 'stats'; data: StatsResult }
  | { type: 'status'; data: StatusResult }
  | { type: 'suggest'; data: SuggestResult }
  | { type: 'retrieve'; data: RetrieveResult }
  | { type: 'dedupe'; data: DedupeResult }
  | { type: 'raw'; data: unknown[] }
