import { NextRequest } from 'next/server'
import { afterEach, describe, expect, it, vi } from 'vitest'

const queryMock = vi.fn()

vi.mock('@/lib/server/pg-pool', () => ({
  getJobsPgPool: () => ({
    query: queryMock,
  }),
}))

describe('job detail route', () => {
  afterEach(() => {
    queryMock.mockReset()
  })

  it('returns refresh job detail when id exists in axon_refresh_jobs', async () => {
    const id = '11111111-1111-4111-8111-111111111111'
    const now = new Date('2026-03-05T12:00:00Z')

    queryMock.mockImplementation(async (sql: string) => {
      if (sql.includes('FROM axon_crawl_jobs')) return { rows: [] }
      if (sql.includes('FROM axon_embed_jobs')) return { rows: [] }
      if (sql.includes('FROM axon_extract_jobs')) return { rows: [] }
      if (sql.includes('FROM axon_ingest_jobs')) return { rows: [] }
      if (sql.includes('FROM axon_refresh_jobs')) {
        return {
          rows: [
            {
              id,
              status: 'completed',
              created_at: now,
              started_at: now,
              finished_at: now,
              error_text: null,
              urls_json: ['https://example.com/docs'],
              result_json: {
                checked: 1,
                changed: 1,
                unchanged: 0,
                not_modified: 0,
                failed: 0,
                total: 1,
              },
              config_json: { collection: 'cortex' },
            },
          ],
        }
      }
      return { rows: [] }
    })

    const mod = await import('@/app/api/jobs/[id]/route')
    const req = new NextRequest(`http://localhost/api/jobs/${id}`)
    const res = await mod.GET(req, { params: Promise.resolve({ id }) })
    expect(res.status).toBe(200)
    const body = (await res.json()) as { type: string; id: string }
    expect(body.id).toBe(id)
    expect(body.type).toBe('refresh')
  })

  it('normalizeOutputDirForWeb passes through unified paths', async () => {
    const mod = await import('@/app/api/jobs/[id]/route')
    expect(mod.normalizeOutputDirForWeb('/data/axon/output/domains/x/sync')).toBe(
      '/data/axon/output/domains/x/sync',
    )
    expect(mod.normalizeOutputDirForWeb(null)).toBeNull()
  })
})
