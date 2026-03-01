'use client'

import { useCallback, useMemo, useState } from 'react'
import { ScrollArea } from '@/components/ui/scroll-area'
import { fileDownloadUrl } from '@/lib/download-urls'
import type { CrawlFile } from '@/lib/ws-protocol'

interface ExtractedSectionProps {
  files: CrawlFile[]
  selectedFile: string | null
  onSelectFile: (relativePath: string) => void
  jobId?: string | null
}

function extractDomain(url: string): string {
  try {
    return new URL(url).hostname
  } catch {
    return 'unknown'
  }
}

export function displayName(url: string, relativePath: string): string {
  try {
    const parsed = new URL(url)
    const segments = parsed.pathname.split('/').filter(Boolean)
    const last = segments[segments.length - 1]
    if (last) {
      return last
        .replace(/\.(html?|md|txt)$/i, '')
        .replace(/[-_]+/g, ' ')
        .replace(/\b\w/g, (c) => c.toUpperCase())
    }
  } catch {
    /* invalid URL — fall through */
  }
  const parts = relativePath.split('/')
  const filename = parts[parts.length - 1] || relativePath
  return filename
    .replace(/\.md$/, '')
    .replace(/^\d+-/, '')
    .replace(/[-_]+/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase())
}

export function breadcrumb(url: string): string {
  try {
    const parsed = new URL(url)
    const segments = parsed.pathname.split('/').filter(Boolean)
    if (segments.length > 1) {
      return segments.slice(0, -1).join(' / ')
    }
  } catch {
    /* ignore */
  }
  return ''
}

export function ExtractedSection({
  files,
  selectedFile,
  onSelectFile,
  jobId,
}: ExtractedSectionProps) {
  const [filter, setFilter] = useState('')

  const domain = useMemo(() => {
    if (files.length === 0) return ''
    return extractDomain(files[0].url)
  }, [files])

  const filteredFiles = useMemo(() => {
    if (!filter.trim()) return files
    const q = filter.toLowerCase()
    return files.filter(
      (f) =>
        displayName(f.url, f.relative_path).toLowerCase().includes(q) ||
        f.url.toLowerCase().includes(q),
    )
  }, [files, filter])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent, relativePath: string) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault()
        onSelectFile(relativePath)
      }
    },
    [onSelectFile],
  )

  if (files.length === 0) {
    return (
      <div className="px-3 py-6 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
        No crawled files yet
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-1">
      {domain && (
        <div className="truncate px-3 text-[length:var(--text-xs)] text-[var(--text-dim)]">
          {domain} &mdash; {files.length} pages
        </div>
      )}
      <div className="px-2">
        <input
          type="text"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="Filter pages..."
          className="w-full rounded bg-[var(--surface-elevated)] px-2 py-1.5 text-[length:var(--text-md)] text-[var(--text-secondary)] placeholder-[var(--text-dim)] outline-none ring-1 ring-[var(--border-subtle)] transition-all focus:ring-[var(--focus-ring-color)]"
        />
      </div>
      <ScrollArea className="max-h-[30vh]">
        {filteredFiles.map((file) => {
          const isActive = file.relative_path === selectedFile
          const name = displayName(file.url, file.relative_path)
          const crumb = breadcrumb(file.url)
          return (
            <div
              key={file.relative_path}
              role="button"
              tabIndex={0}
              onClick={() => onSelectFile(file.relative_path)}
              onKeyDown={(e) => handleKeyDown(e, file.relative_path)}
              className={`cursor-pointer border-b border-[var(--border-subtle)] px-3 py-2 transition-colors focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-[-2px] focus-visible:rounded-sm ${
                isActive
                  ? 'border-l-2 border-l-[var(--axon-secondary)] bg-[rgba(255,135,175,0.08)]'
                  : 'border-l-2 border-l-transparent hover:bg-[var(--surface-float)]'
              }`}
            >
              <div className="flex items-start justify-between gap-1.5">
                <div className="min-w-0 flex-1">
                  <div
                    className={`truncate text-[length:var(--text-md)] font-medium leading-[var(--leading-tight)] ${isActive ? 'text-[var(--axon-primary)]' : 'text-[var(--text-secondary)]'}`}
                  >
                    {name}
                  </div>
                  {crumb && (
                    <div className="truncate text-[length:var(--text-xs)] text-[var(--text-dim)]">
                      {crumb}
                    </div>
                  )}
                </div>
                <div className="mt-0.5 flex flex-shrink-0 items-center gap-1">
                  <span className="text-[length:var(--text-xs)] tabular-nums text-[var(--text-dim)]">
                    {file.markdown_chars > 1000
                      ? `${(file.markdown_chars / 1000).toFixed(1)}k`
                      : file.markdown_chars}
                  </span>
                  {jobId && (
                    <a
                      href={fileDownloadUrl(jobId, file.relative_path)}
                      download
                      aria-label={`Download ${name}`}
                      onClick={(e) => e.stopPropagation()}
                      className="rounded p-0.5 text-[var(--text-dim)] transition-colors hover:text-[var(--axon-primary)] focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
                      title="Download file"
                    >
                      <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth={2}
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        className="size-2.5"
                      >
                        <path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                      </svg>
                    </a>
                  )}
                </div>
              </div>
            </div>
          )
        })}
        {filteredFiles.length === 0 && filter && (
          <div className="px-3 py-4 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
            No matches
          </div>
        )}
      </ScrollArea>
    </div>
  )
}
