/**
 * Normalizers that take raw stdout JSON + command mode and return a typed
 * discriminated union for the renderer dispatch layer.
 */

import type {
  AskResult,
  DebugResult,
  DedupeResult,
  DoctorResult,
  DomainsResult,
  EvaluateResult,
  MapResult,
  NormalizedResult,
  QueryResult,
  RetrieveResult,
  SourcesResult,
  StatsResult,
  StatusResult,
  SuggestResult,
} from '@/lib/result-types'

// ---------------------------------------------------------------------------
// Helpers — lightweight shape checks (duck-typing, not full Zod)
// ---------------------------------------------------------------------------

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

function hasKeys(v: unknown, keys: string[]): v is Record<string, unknown> {
  if (!isRecord(v)) return false
  return keys.every((k) => k in v)
}

function first(arr: unknown[]): unknown {
  return arr[0]
}

// ---------------------------------------------------------------------------
// Per-command normalizers
// ---------------------------------------------------------------------------

function normalizeQuery(items: unknown[]): NormalizedResult {
  // Each stdout line is a separate QueryResult
  const valid = items.every((item) => hasKeys(item, ['rank', 'score', 'url', 'snippet']))
  if (!valid || items.length === 0) return { type: 'raw', data: items }
  return { type: 'query', data: items as unknown as QueryResult[] }
}

function normalizeAsk(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!hasKeys(obj, ['query', 'answer'])) return { type: 'raw', data: items }
  return { type: 'ask', data: obj as unknown as AskResult }
}

function normalizeEvaluate(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!hasKeys(obj, ['query', 'rag_answer', 'baseline_answer'])) return { type: 'raw', data: items }
  return { type: 'evaluate', data: obj as unknown as EvaluateResult }
}

function normalizeDoctor(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!hasKeys(obj, ['services', 'all_ok'])) return { type: 'raw', data: items }
  return { type: 'doctor', data: obj as unknown as DoctorResult }
}

function normalizeDebug(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!hasKeys(obj, ['doctor_report', 'llm_debug'])) return { type: 'raw', data: items }
  return { type: 'debug', data: obj as unknown as DebugResult }
}

function normalizeMap(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!hasKeys(obj, ['url', 'urls'])) return { type: 'raw', data: items }
  return { type: 'map', data: obj as unknown as MapResult }
}

function normalizeSources(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!isRecord(obj)) return { type: 'raw', data: items }
  // sources is a flat {url: count} map — verify at least one value is a number
  const values = Object.values(obj)
  if (values.length > 0 && typeof values[0] !== 'number') return { type: 'raw', data: items }
  return { type: 'sources', data: obj as SourcesResult }
}

function normalizeDomains(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!isRecord(obj)) return { type: 'raw', data: items }
  return { type: 'domains', data: obj as DomainsResult }
}

function normalizeStats(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!hasKeys(obj, ['collection', 'points_count'])) return { type: 'raw', data: items }
  return { type: 'stats', data: obj as unknown as StatsResult }
}

function normalizeStatus(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!isRecord(obj)) return { type: 'raw', data: items }
  // Accept both canonical and relaxed status payload shapes.
  // Canonical: local_*_jobs keys from `axon status --json`.
  const hasCanonicalKeys = ['local_crawl_jobs', 'local_extract_jobs', 'local_embed_jobs', 'local_ingest_jobs']
    .some((k) => Array.isArray(obj[k]))

  // Relaxed: any "*_jobs" array key (forward/backward compatibility).
  const hasAnyJobsArray = Object.keys(obj).some((k) => k.endsWith('_jobs') && Array.isArray(obj[k]))

  if (!hasCanonicalKeys && !hasAnyJobsArray) return { type: 'raw', data: items }

  // Normalize into the renderer's expected shape so missing keys don't break UI.
  const normalized: StatusResult = {
    local_crawl_jobs: ((obj.local_crawl_jobs as unknown[]) ?? ((obj.crawl_jobs as unknown[]) ?? [])) as StatusResult['local_crawl_jobs'],
    local_extract_jobs: ((obj.local_extract_jobs as unknown[]) ?? ((obj.extract_jobs as unknown[]) ?? [])) as StatusResult['local_extract_jobs'],
    local_embed_jobs: ((obj.local_embed_jobs as unknown[]) ?? ((obj.embed_jobs as unknown[]) ?? [])) as StatusResult['local_embed_jobs'],
    local_ingest_jobs: ((obj.local_ingest_jobs as unknown[]) ?? ((obj.ingest_jobs as unknown[]) ?? [])) as StatusResult['local_ingest_jobs'],
  }

  return { type: 'status', data: normalized }
}

function normalizeSuggest(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!hasKeys(obj, ['suggestions'])) return { type: 'raw', data: items }
  return { type: 'suggest', data: obj as unknown as SuggestResult }
}

function normalizeRetrieve(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!hasKeys(obj, ['url', 'chunks', 'content'])) return { type: 'raw', data: items }
  return { type: 'retrieve', data: obj as unknown as RetrieveResult }
}

function normalizeDedupe(items: unknown[]): NormalizedResult {
  const obj = first(items)
  if (!hasKeys(obj, ['duplicate_groups', 'deleted'])) return { type: 'raw', data: items }
  return { type: 'dedupe', data: obj as unknown as DedupeResult }
}

// ---------------------------------------------------------------------------
// Main dispatch
// ---------------------------------------------------------------------------

const NORMALIZERS: Record<string, (items: unknown[]) => NormalizedResult> = {
  query: normalizeQuery,
  ask: normalizeAsk,
  evaluate: normalizeEvaluate,
  doctor: normalizeDoctor,
  debug: normalizeDebug,
  map: normalizeMap,
  sources: normalizeSources,
  domains: normalizeDomains,
  stats: normalizeStats,
  status: normalizeStatus,
  suggest: normalizeSuggest,
  retrieve: normalizeRetrieve,
  dedupe: normalizeDedupe,
}

export function normalizeResult(commandMode: string, stdoutJson: unknown[]): NormalizedResult {
  if (stdoutJson.length === 0) return { type: 'raw', data: stdoutJson }

  const normalizer = NORMALIZERS[commandMode]
  if (!normalizer) return { type: 'raw', data: stdoutJson }

  return normalizer(stdoutJson)
}
