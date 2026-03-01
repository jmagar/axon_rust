export function packMdUrl(jobId: string): string {
  return `/download/${encodeURIComponent(jobId)}/pack.md`
}

export function packXmlUrl(jobId: string): string {
  return `/download/${encodeURIComponent(jobId)}/pack.xml`
}

export function archiveZipUrl(jobId: string): string {
  return `/download/${encodeURIComponent(jobId)}/archive.zip`
}

export function fileDownloadUrl(jobId: string, relPath: string): string {
  const encoded = relPath
    .split('/')
    .map((segment) => encodeURIComponent(segment))
    .join('/')
  return `/download/${encodeURIComponent(jobId)}/file/${encoded}`
}
