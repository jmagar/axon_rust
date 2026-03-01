import { describe, expect, it } from 'vitest'
import { normalizeResult } from '@/lib/result-normalizers'

describe('result normalizers status compatibility', () => {
  it('accepts canonical local_*_jobs shape', () => {
    const input = [
      {
        local_crawl_jobs: [{ id: 'c1', status: 'running' }],
        local_extract_jobs: [{ id: 'e1', status: 'pending' }],
        local_embed_jobs: [],
        local_ingest_jobs: [{ id: 'i1', status: 'completed' }],
      },
    ]

    const normalized = normalizeResult('status', input)
    expect(normalized.type).toBe('status')
    if (normalized.type !== 'status') return

    expect(normalized.data.local_crawl_jobs).toHaveLength(1)
    expect(normalized.data.local_extract_jobs).toHaveLength(1)
    expect(normalized.data.local_embed_jobs).toHaveLength(0)
    expect(normalized.data.local_ingest_jobs).toHaveLength(1)
  })

  it('accepts relaxed *_jobs shape and maps into canonical keys', () => {
    const input = [
      {
        crawl_jobs: [{ id: 'c1', status: 'running' }],
        extract_jobs: [{ id: 'e1', status: 'pending' }],
        ingest_jobs: [{ id: 'i1', status: 'completed' }],
      },
    ]

    const normalized = normalizeResult('status', input)
    expect(normalized.type).toBe('status')
    if (normalized.type !== 'status') return

    expect(normalized.data.local_crawl_jobs).toHaveLength(1)
    expect(normalized.data.local_extract_jobs).toHaveLength(1)
    expect(normalized.data.local_embed_jobs).toHaveLength(0)
    expect(normalized.data.local_ingest_jobs).toHaveLength(1)
  })

  it('falls back to raw for non-status payloads', () => {
    const input = [{ id: 'job-1', status: 'running' }]
    const normalized = normalizeResult('status', input)

    expect(normalized).toEqual({ type: 'raw', data: input })
  })
})
