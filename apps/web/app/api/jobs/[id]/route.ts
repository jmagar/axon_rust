import { createReadStream } from 'node:fs'
import { access } from 'node:fs/promises'
import { createInterface } from 'node:readline'
import { type NextRequest, NextResponse } from 'next/server'
import { apiError } from '@/lib/server/api-error'
import { type JobStatus, type JobType, safeStatus } from '@/lib/server/job-types'
import { getJobsPgPool } from '@/lib/server/pg-pool'

interface CrawlMarkdownFile {
  url: string
  relativePath: string
  markdownChars: number
  changed: boolean | null
}

export interface JobDetail {
  id: string
  type: JobType
  status: JobStatus
  success: boolean | null
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
  errorPages: number | null
  wafBlockedPages: number | null
  cacheHit: boolean | null
  outputDir: string | null
  staleUrlsDeleted: number | null
  thinUrls: string[] | null
  wafBlockedUrls: string[] | null
  observedUrls: string[] | null
  markdownFiles: CrawlMarkdownFile[] | null
  // embed-specific
  docsEmbedded: number | null
  chunksEmbedded: number | null
  // extract-specific
  urls: string[] | null
  // raw JSON for advanced view
  resultJson: Record<string, unknown> | null
  configJson: Record<string, unknown> | null
}

function boolOrNull(value: unknown): boolean | null {
  return typeof value === 'boolean' ? value : null
}

function numberOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function stringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value.filter((v): v is string => typeof v === 'string')
}

async function readCrawlManifest(outputDir: string): Promise<CrawlMarkdownFile[]> {
  const manifestPath = `${outputDir}/manifest.jsonl`
  try {
    await access(manifestPath)
  } catch {
    return []
  }

  const files: CrawlMarkdownFile[] = []
  const stream = createReadStream(manifestPath, { encoding: 'utf8' })
  const reader = createInterface({ input: stream, crlfDelay: Infinity })
  for await (const rawLine of reader) {
    const line = rawLine.trim()
    if (!line) continue
    let parsed: unknown
    try {
      parsed = JSON.parse(line)
    } catch {
      continue
    }
    if (!parsed || typeof parsed !== 'object') continue
    const obj = parsed as Record<string, unknown>
    const url = typeof obj.url === 'string' ? obj.url : null
    const relativePath =
      typeof obj.relative_path === 'string'
        ? obj.relative_path
        : typeof obj.file_path === 'string'
          ? obj.file_path
          : null
    if (!url || !relativePath) continue
    files.push({
      url,
      relativePath,
      markdownChars: numberOrNull(obj.markdown_chars) ?? 0,
      changed: boolOrNull(obj.changed),
    })
  }

  return files
}

async function findCrawlJob(id: string, includeArtifacts: boolean): Promise<JobDetail | null> {
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
  const outputDir = (res.output_dir as string) ?? null
  const manifestFiles = includeArtifacts && outputDir ? await readCrawlManifest(outputDir) : []
  const thinUrls = stringArray(res.thin_urls)
  const wafBlockedUrls = stringArray(res.waf_blocked_urls)
  const observedUrlSet = new Set<string>([
    ...manifestFiles.map((entry) => entry.url),
    ...thinUrls,
    ...wafBlockedUrls,
  ])
  const createdAt = row.created_at as Date
  const startedAt = row.started_at as Date | null
  const finishedAt = row.finished_at as Date | null
  return {
    id: row.id as string,
    type: 'crawl',
    status: safeStatus(row.status as string),
    success:
      row.status === 'completed'
        ? true
        : row.status === 'failed' || row.status === 'canceled'
          ? false
          : null,
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
    errorPages: res.error_pages != null ? Number(res.error_pages) : null,
    wafBlockedPages: res.waf_blocked_pages != null ? Number(res.waf_blocked_pages) : null,
    cacheHit: res.cache_hit != null ? Boolean(res.cache_hit) : null,
    outputDir,
    staleUrlsDeleted: res.stale_urls_deleted != null ? Number(res.stale_urls_deleted) : null,
    thinUrls,
    wafBlockedUrls,
    observedUrls: includeArtifacts ? [...observedUrlSet] : null,
    markdownFiles: includeArtifacts ? manifestFiles : null,
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
    success:
      row.status === 'completed'
        ? true
        : row.status === 'failed' || row.status === 'canceled'
          ? false
          : null,
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
    errorPages: null,
    wafBlockedPages: null,
    cacheHit: null,
    outputDir: null,
    staleUrlsDeleted: null,
    thinUrls: null,
    wafBlockedUrls: null,
    observedUrls: null,
    markdownFiles: null,
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
    success:
      row.status === 'completed'
        ? true
        : row.status === 'failed' || row.status === 'canceled'
          ? false
          : null,
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
    errorPages: null,
    wafBlockedPages: null,
    cacheHit: null,
    outputDir: null,
    staleUrlsDeleted: null,
    thinUrls: null,
    wafBlockedUrls: null,
    observedUrls: null,
    markdownFiles: null,
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
    success:
      row.status === 'completed'
        ? true
        : row.status === 'failed' || row.status === 'canceled'
          ? false
          : null,
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
    errorPages: null,
    wafBlockedPages: null,
    cacheHit: null,
    outputDir: null,
    staleUrlsDeleted: null,
    thinUrls: null,
    wafBlockedUrls: null,
    observedUrls: null,
    markdownFiles: null,
    docsEmbedded: null,
    chunksEmbedded: null,
    urls: null,
    resultJson: res,
    configJson: cfg,
  }
}

export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> },
): Promise<NextResponse> {
  const { id } = await params
  const includeArtifacts = req.nextUrl.searchParams.get('includeArtifacts') === '1'

  if (!id || !/^[0-9a-f-]{36}$/i.test(id)) {
    return apiError(400, 'Invalid job ID')
  }

  try {
    // Search all tables — first match wins
    const job =
      (await findCrawlJob(id, includeArtifacts)) ??
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
