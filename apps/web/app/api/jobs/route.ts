import { type NextRequest, NextResponse } from 'next/server'
import { Pool } from 'pg'

// ── Types ──────────────────────────────────────────────────────────────────────

export type JobType = 'crawl' | 'extract' | 'embed' | 'ingest'
export type JobStatus = 'pending' | 'running' | 'completed' | 'failed' | 'canceled'

export interface Job {
  id: string
  type: JobType
  status: JobStatus
  target: string
  collection: string | null
  createdAt: string
  startedAt: string | null
  finishedAt: string | null
  errorText: string | null
}

interface JobsResponse {
  jobs: Job[]
  total: number
  hasMore: boolean
}

// ── DB pool ────────────────────────────────────────────────────────────────────

const pool = new Pool({
  connectionString: process.env.AXON_PG_URL ?? 'postgresql://axon:postgres@axon-postgres:5432/axon',
})

// ── Helpers ────────────────────────────────────────────────────────────────────

function safeStatus(s: string): JobStatus {
  const valid: JobStatus[] = ['pending', 'running', 'completed', 'failed', 'canceled']
  return valid.includes(s as JobStatus) ? (s as JobStatus) : 'pending'
}

function truncate(s: string | null | undefined, max = 120): string {
  if (!s) return '—'
  return s.length > max ? `${s.slice(0, max)}…` : s
}

// ── Query builders ─────────────────────────────────────────────────────────────

type StatusFilter = 'all' | 'active' | 'pending' | 'completed' | 'failed'

function statusWhere(filter: StatusFilter): string {
  switch (filter) {
    case 'active':
      return `status IN ('pending','running')`
    case 'pending':
      return `status = 'pending'`
    case 'completed':
      return `status = 'completed'`
    case 'failed':
      return `status IN ('failed','canceled')`
    default:
      return '1=1'
  }
}

async function queryCrawl(statusFilter: StatusFilter, limit: number, offset: number) {
  const where = statusWhere(statusFilter)
  const rows = await pool.query(
    `SELECT id, url, status, created_at, started_at, finished_at, error_text,
            config_json->>'collection' AS collection,
            COUNT(*) OVER() AS total
     FROM axon_crawl_jobs
     WHERE ${where}
     ORDER BY created_at DESC
     LIMIT $1 OFFSET $2`,
    [limit, offset],
  )
  return {
    jobs: rows.rows.map((r) => ({
      id: r.id as string,
      type: 'crawl' as JobType,
      status: safeStatus(r.status as string),
      target: truncate(r.url as string),
      collection: r.collection as string | null,
      createdAt: (r.created_at as Date).toISOString(),
      startedAt: r.started_at ? (r.started_at as Date).toISOString() : null,
      finishedAt: r.finished_at ? (r.finished_at as Date).toISOString() : null,
      errorText: r.error_text as string | null,
    })),
    total: Number((rows.rows[0] as { total?: string } | undefined)?.total ?? 0),
  }
}

async function queryExtract(statusFilter: StatusFilter, limit: number, offset: number) {
  const where = statusWhere(statusFilter)
  const rows = await pool.query(
    `SELECT id, urls_json, status, created_at, started_at, finished_at, error_text
     FROM axon_extract_jobs
     WHERE ${where}
     ORDER BY created_at DESC
     LIMIT $1 OFFSET $2`,
    [limit, offset],
  )
  const count = await pool.query(`SELECT COUNT(*) FROM axon_extract_jobs WHERE ${where}`)
  return {
    jobs: rows.rows.map((r) => {
      const urls = Array.isArray(r.urls_json) ? (r.urls_json as string[]) : []
      const first = urls[0] ?? '—'
      const label = urls.length > 1 ? `${first} (+${urls.length - 1})` : first
      return {
        id: r.id as string,
        type: 'extract' as JobType,
        status: safeStatus(r.status as string),
        target: truncate(label),
        collection: null,
        createdAt: (r.created_at as Date).toISOString(),
        startedAt: r.started_at ? (r.started_at as Date).toISOString() : null,
        finishedAt: r.finished_at ? (r.finished_at as Date).toISOString() : null,
        errorText: r.error_text as string | null,
      }
    }),
    total: Number((count.rows[0] as { count: string }).count),
  }
}

async function queryEmbed(statusFilter: StatusFilter, limit: number, offset: number) {
  const where = statusWhere(statusFilter)
  const rows = await pool.query(
    `SELECT id, input_text, status, created_at, started_at, finished_at, error_text, config_json->>'collection' AS collection
     FROM axon_embed_jobs
     WHERE ${where}
     ORDER BY created_at DESC
     LIMIT $1 OFFSET $2`,
    [limit, offset],
  )
  const count = await pool.query(`SELECT COUNT(*) FROM axon_embed_jobs WHERE ${where}`)
  return {
    jobs: rows.rows.map((r) => ({
      id: r.id as string,
      type: 'embed' as JobType,
      status: safeStatus(r.status as string),
      target: truncate(r.input_text as string),
      collection: r.collection as string | null,
      createdAt: (r.created_at as Date).toISOString(),
      startedAt: r.started_at ? (r.started_at as Date).toISOString() : null,
      finishedAt: r.finished_at ? (r.finished_at as Date).toISOString() : null,
      errorText: r.error_text as string | null,
    })),
    total: Number((count.rows[0] as { count: string }).count),
  }
}

async function queryIngest(statusFilter: StatusFilter, limit: number, offset: number) {
  const where = statusWhere(statusFilter)
  const rows = await pool.query(
    `SELECT id, source_type, target, status, created_at, started_at, finished_at, error_text
     FROM axon_ingest_jobs
     WHERE ${where}
     ORDER BY created_at DESC
     LIMIT $1 OFFSET $2`,
    [limit, offset],
  )
  const count = await pool.query(`SELECT COUNT(*) FROM axon_ingest_jobs WHERE ${where}`)
  return {
    jobs: rows.rows.map((r) => ({
      id: r.id as string,
      type: 'ingest' as JobType,
      status: safeStatus(r.status as string),
      target: truncate(`${r.source_type as string}: ${r.target as string}`),
      collection: null,
      createdAt: (r.created_at as Date).toISOString(),
      startedAt: r.started_at ? (r.started_at as Date).toISOString() : null,
      finishedAt: r.finished_at ? (r.finished_at as Date).toISOString() : null,
      errorText: r.error_text as string | null,
    })),
    total: Number((count.rows[0] as { count: string }).count),
  }
}

// ── GET /api/jobs ──────────────────────────────────────────────────────────────

export async function GET(req: NextRequest): Promise<NextResponse> {
  const { searchParams } = req.nextUrl
  const type = (searchParams.get('type') ?? 'all') as 'all' | JobType
  const statusRaw = (searchParams.get('status') ?? 'all') as StatusFilter
  const limit = Math.min(Math.max(Number(searchParams.get('limit') ?? '50'), 1), 200)
  const offset = Math.max(Number(searchParams.get('offset') ?? '0'), 0)

  try {
    let jobs: Job[] = []
    let total = 0

    if (type === 'all') {
      // Fetch from all 4 tables, merge, sort by createdAt desc, paginate
      const [crawl, extract, embed, ingest] = await Promise.all([
        queryCrawl(statusRaw, 500, 0),
        queryExtract(statusRaw, 500, 0),
        queryEmbed(statusRaw, 500, 0),
        queryIngest(statusRaw, 500, 0),
      ])
      const all = [...crawl.jobs, ...extract.jobs, ...embed.jobs, ...ingest.jobs].sort((a, b) =>
        b.createdAt.localeCompare(a.createdAt),
      )
      total = all.length
      jobs = all.slice(offset, offset + limit)
    } else {
      const query =
        type === 'crawl'
          ? queryCrawl
          : type === 'extract'
            ? queryExtract
            : type === 'embed'
              ? queryEmbed
              : queryIngest
      const result = await query(statusRaw, limit, offset)
      jobs = result.jobs
      total = result.total
    }

    const response: JobsResponse = {
      jobs,
      total,
      hasMore: offset + jobs.length < total,
    }
    return NextResponse.json(response)
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Database error'
    return NextResponse.json({ error: message }, { status: 500 })
  }
}

// ── POST /api/jobs/cancel ──────────────────────────────────────────────────────

export async function POST(): Promise<NextResponse> {
  return NextResponse.json(
    { ok: false, message: 'Cancel not yet supported from UI' },
    { status: 200 },
  )
}
