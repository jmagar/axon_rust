import { describe, expect, it } from 'vitest'
import { archiveZipUrl, fileDownloadUrl, packMdUrl, packXmlUrl } from '@/lib/download-urls'

describe('download URL helpers', () => {
  const jobId = 'abc-123'

  it('packMdUrl encodes job ID and appends pack.md', () => {
    expect(packMdUrl(jobId)).toBe('/download/abc-123/pack.md')
  })

  it('packXmlUrl encodes job ID and appends pack.xml', () => {
    expect(packXmlUrl(jobId)).toBe('/download/abc-123/pack.xml')
  })

  it('archiveZipUrl encodes job ID and appends archive.zip', () => {
    expect(archiveZipUrl(jobId)).toBe('/download/abc-123/archive.zip')
  })

  it('encodes special characters in job ID', () => {
    const special = 'job/with spaces&stuff'
    expect(packMdUrl(special)).toBe(`/download/${encodeURIComponent(special)}/pack.md`)
  })

  it('fileDownloadUrl encodes each path segment independently', () => {
    const result = fileDownloadUrl(jobId, 'docs/output/file name.md')
    expect(result).toBe('/download/abc-123/file/docs/output/file%20name.md')
  })

  it('fileDownloadUrl handles single segment path', () => {
    expect(fileDownloadUrl(jobId, 'readme.md')).toBe('/download/abc-123/file/readme.md')
  })

  it('fileDownloadUrl handles deeply nested paths', () => {
    const result = fileDownloadUrl(jobId, 'a/b/c/d.txt')
    expect(result).toBe('/download/abc-123/file/a/b/c/d.txt')
  })
})
