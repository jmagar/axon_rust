import { type NextRequest, NextResponse } from 'next/server'
import { apiError } from '@/lib/server/api-error'
import { getJobsPgPool } from '@/lib/server/pg-pool'
import type { JobStatus, JobType } from '../route'

function safeStatus(s: string): JobStatus {
  const valid: JobStatus[] = ['pending', 'running', 'completed', 'failed', 'canceled']
  return valid.includes(s as JobStatus) ? (s as JobStatus) : 'pending'
}

export interface JobDetail {
  id: string
  type: JobType
  status: JobStatus
  target: string
  collection: string | null
  renderMode: string | null
  maxDepth: number | null
  maxPages: number | null
  embed: boolean | null
  createdAt: string
  startedAt: string | null
  finishedAt: string | null
  elapsedMs: number | null
  errorText: string | null
  // crawl-specific
  pagesCrawled: number | null
  pagesDiscovered: number | null
  mdCreated: number | null
  thinMd: number | null
  filteredUrls: number | null
  cacheHit: boolean | null
  outputDir: string | null
  // embed-specific
  docsEmbedded: number | null
  chunksEmbedded: number | null
  // extract-specific
  urls: string[] | null
  // raw JSON for advanced view
  resultJson: Record<string, unknown> | null
  configJson: Record<string, unknown> | null
}

async function findCrawlJob(id: string): Promise<JobDetail | null> {
  const r = await getJobsPgPool().query(
    `SELECT id, url, status, created_at, started_at, finished_at, error_text,
            result_json, config_json
     FROM axon_crawl_jobs WHERE id = $1`,
    [id],
  )
  if (!r.rows.length) return null
  const row = r.rows[0]
  const res = (row.result_json ?? {}) as Record<string, unknown>
  const cfg = (row.config_json ?? {}) as Record<string, unknown>
  const createdAt = row.created_at as Date
  const startedAt = row.started_at as Date | null
  const finishedAt = row.finished_at as Date | null
  return {
    id: row.id as string,
    type: 'crawl',
    status: safeStatus(row.status as string),
    target: row.url as string,
    collection: (cfg.collection as string) ?? null,
    renderMode: (cfg.render_mode as string) ?? null,
    maxDepth: cfg.max_depth != null ? Number(cfg.max_depth) : null,
    maxPages: cfg.max_pages != null ? Number(cfg.max_pages) : null,
    embed: cfg.embed != null ? Boolean(cfg.embed) : null,
    createdAt: createdAt.toISOString(),
    startedAt: startedAt ? startedAt.toISOString() : null,
    finishedAt: finishedAt ? finishedAt.toISOString() : null,
    elapsedMs: res.elapsed_ms != null ? Number(res.elapsed_ms) : null,
    errorText: row.error_text as string | null,
    pagesCrawled: res.pages_crawled != null ? Number(res.pages_crawled) : null,
    pagesDiscovered: res.pages_discovered != null ? Number(res.pages_discovered) : null,
    mdCreated: res.md_created != null ? Number(res.md_created) : null,
    thinMd: res.thin_md != null ? Number(res.thin_md) : null,
    filteredUrls: res.filtered_urls != null ? Number(res.filtered_urls) : null,
    cacheHit: res.cache_hit != null ? Boolean(res.cache_hit) : null,
    outputDir: (res.output_dir as string) ?? null,
    docsEmbedded: null,
    chunksEmbedded: null,
    urls: null,
    resultJson: res,
    configJson: cfg,
  }
}

async function findEmbedJob(id: string): Promise<JobDetail | null> {
  const r = await getJobsPgPool().query(
    `SELECT id, input_text, status, created_at, started_at, finished_at, error_text,
            result_json, config_json
     FROM axon_embed_jobs WHERE id = $1`,
    [id],
  )
  if (!r.rows.length) return null
  const row = r.rows[0]
  const res = (row.result_json ?? {}) as Record<string, unknown>
  const cfg = (row.config_json ?? {}) as Record<string, unknown>
  const createdAt = row.created_at as Date
  const startedAt = row.started_at as Date | null
  const finishedAt = row.finished_at as Date | null
  return {
    id: row.id as string,
    type: 'embed',
    status: safeStatus(row.status as string),
    target: row.input_text as string,
    collection: (res.collection as string) ?? (cfg.collection as string) ?? null,
    renderMode: null,
    maxDepth: null,
    maxPages: null,
    embed: null,
    createdAt: createdAt.toISOString(),
    startedAt: startedAt ? startedAt.toISOString() : null,
    finishedAt: finishedAt ? finishedAt.toISOString() : null,
    elapsedMs: null,
    errorText: row.error_text as string | null,
    pagesCrawled: null,
    pagesDiscovered: null,
    mdCreated: null,
    thinMd: null,
    filteredUrls: null,
    cacheHit: null,
    outputDir: null,
    docsEmbedded: res.docs_embedded != null ? Number(res.docs_embedded) : null,
    chunksEmbedded: res.chunks_embedded != null ? Number(res.chunks_embedded) : null,
    urls: null,
    resultJson: res,
    configJson: cfg,
  }
}

async function findExtractJob(id: string): Promise<JobDetail | null> {
  const r = await getJobsPgPool().query(
    `SELECT id, urls_json, status, created_at, started_at, finished_at, error_text,
            result_json, config_json
     FROM axon_extract_jobs WHERE id = $1`,
    [id],
  )
  if (!r.rows.length) return null
  const row = r.rows[0]
  const res = (row.result_json ?? {}) as Record<string, unknown>
  const cfg = (row.config_json ?? {}) as Record<string, unknown>
  const urls = Array.isArray(row.urls_json) ? (row.urls_json as string[]) : []
  const createdAt = row.created_at as Date
  const startedAt = row.started_at as Date | null
  const finishedAt = row.finished_at as Date | null
  return {
    id: row.id as string,
    type: 'extract',
    status: safeStatus(row.status as string),
    target: urls[0] ?? '—',
    collection: (cfg.collection as string) ?? null,
    renderMode: null,
    maxDepth: null,
    maxPages: null,
    embed: null,
    createdAt: createdAt.toISOString(),
    startedAt: startedAt ? startedAt.toISOString() : null,
    finishedAt: finishedAt ? finishedAt.toISOString() : null,
    elapsedMs: null,
    errorText: row.error_text as string | null,
    pagesCrawled: null,
    pagesDiscovered: null,
    mdCreated: null,
    thinMd: null,
    filteredUrls: null,
    cacheHit: null,
    outputDir: null,
    docsEmbedded: null,
    chunksEmbedded: null,
    urls,
    resultJson: res,
    configJson: cfg,
  }
}

async function findIngestJob(id: string): Promise<JobDetail | null> {
  const r = await getJobsPgPool().query(
    `SELECT id, source_type, target, status, created_at, started_at, finished_at, error_text,
            result_json, config_json
     FROM axon_ingest_jobs WHERE id = $1`,
    [id],
  )
  if (!r.rows.length) return null
  const row = r.rows[0]
  const res = (row.result_json ?? {}) as Record<string, unknown>
  const cfg = (row.config_json ?? {}) as Record<string, unknown>
  const createdAt = row.created_at as Date
  const startedAt = row.started_at as Date | null
  const finishedAt = row.finished_at as Date | null
  return {
    id: row.id as string,
    type: 'ingest',
    status: safeStatus(row.status as string),
    target: `${row.source_type as string}: ${row.target as string}`,
    collection: null,
    renderMode: null,
    maxDepth: null,
    maxPages: null,
    embed: null,
    createdAt: createdAt.toISOString(),
    startedAt: startedAt ? startedAt.toISOString() : null,
    finishedAt: finishedAt ? finishedAt.toISOString() : null,
    elapsedMs: null,
    errorText: row.error_text as string | null,
    pagesCrawled: null,
    pagesDiscovered: null,
    mdCreated: null,
    thinMd: null,
    filteredUrls: null,
    cacheHit: null,
    outputDir: null,
    docsEmbedded: null,
    chunksEmbedded: null,
    urls: null,
    resultJson: res,
    configJson: cfg,
  }
}

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> },
): Promise<NextResponse> {
  const { id } = await params

  if (!id || !/^[0-9a-f-]{36}$/i.test(id)) {
    return apiError(400, 'Invalid job ID')
  }

  try {
    // Search all tables — first match wins
    const job =
      (await findCrawlJob(id)) ??
      (await findEmbedJob(id)) ??
      (await findExtractJob(id)) ??
      (await findIngestJob(id))

    if (!job) {
      return apiError(404, 'Job not found')
    }

    return NextResponse.json(job)
  } catch (err) {
    console.error('[api/jobs/[id]] database error', err)
    return apiError(500, 'Failed to fetch job details', { code: 'jobs_db_error' })
  }
}
