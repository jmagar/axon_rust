import { describe, expect, it } from 'vitest'
import { selectNormalizedItems } from '@/components/results-panel'

describe('results-panel normalization input', () => {
  it('uses only structured v2 JSON payloads', () => {
    const normalized = selectNormalizedItems([], [])
    expect(normalized).toEqual([])
  })

  it('does not scrape JSON from stdout text fallback', () => {
    const stdoutJson: unknown[] = []
    const stdoutLines = ['{"status":"running","job_id":"job-legacy"}']
    const normalized = selectNormalizedItems(stdoutJson, stdoutLines)
    expect(normalized).toEqual([])
  })

  it('ignores lifecycle-like payloads from job streams', () => {
    const stdoutJson: unknown[] = [{ status: 'running', job_id: 'job-1' }]
    const normalized = selectNormalizedItems(stdoutJson, [])
    expect(normalized).toEqual([])
  })

  it('filters lifecycle-only entries when mixed with structured command output', () => {
    const lifecycle = { job_id: 'job-1', mode: 'crawl', status: 'running', percent: 25 }
    const structured = {
      local_crawl_jobs: [{ id: 'job-1', status: 'running' }],
      local_extract_jobs: [],
      local_embed_jobs: [],
      local_ingest_jobs: [],
    }
    const normalized = selectNormalizedItems([lifecycle, structured], [])
    expect(normalized).toEqual([structured])
  })

  it('keeps non-lifecycle job-shaped outputs for normalizers', () => {
    const commandPayload = { job_id: 'job-1', status: 'running', pages_crawled: 12 }
    const normalized = selectNormalizedItems([commandPayload], [])
    expect(normalized).toEqual([commandPayload])
  })
})
