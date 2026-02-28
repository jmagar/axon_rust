'use client'

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { fileDownloadUrl } from '@/lib/download-urls'
import type { CrawlFile } from '@/lib/ws-protocol'

interface CrawlFileExplorerProps {
  files: CrawlFile[]
  selectedFile: string | null
  onSelectFile: (relativePath: string) => void
  jobId?: string | null
}

/** Extract domain from a URL string. */
function extractDomain(url: string): string {
  try {
    return new URL(url).hostname
  } catch {
    return 'unknown'
  }
}

/** Derive a human-readable page title from the URL path.
 *  e.g. "https://platejs.org/docs/editor-methods" -> "Editor Methods"
 *  Falls back to cleaning the relative_path filename if URL has no usable path. */
function displayName(url: string, relativePath: string): string {
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

/** Derive breadcrumb path from URL (excluding domain and last segment).
 *  e.g. "https://platejs.org/docs/api/editor-methods" -> "docs / api" */
function breadcrumb(url: string): string {
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

export function CrawlFileExplorer({
  files,
  selectedFile,
  onSelectFile,
  jobId,
}: CrawlFileExplorerProps) {
  const [open, setOpen] = useState(false)
  const [filter, setFilter] = useState('')
  const panelRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const initialize = () => {
      setOpen(window.innerWidth >= 768)
    }
    initialize()
    window.addEventListener('resize', initialize)
    return () => window.removeEventListener('resize', initialize)
  }, [])

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

  const handleSelect = useCallback(
    (relativePath: string) => {
      onSelectFile(relativePath)
      // Auto-close on mobile after selection
      if (window.innerWidth < 768) {
        setOpen(false)
      }
    },
    [onSelectFile],
  )

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) setOpen(false)
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [open])

  const fileCountLabel = files.length > 99 ? '99+' : String(files.length)
  const openLabel = `Open files (${files.length})`
  const fileIcon = (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      className="size-3.5"
      aria-hidden="true"
    >
      <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
      <path d="M14 2v4a2 2 0 0 0 2 2h4" />
      <path d="M9 13h6" />
      <path d="M9 17h6" />
    </svg>
  )

  if (!open) {
    return (
      <>
        <div className="hidden w-12 flex-shrink-0 items-start justify-center self-stretch border-r border-[rgba(255,135,175,0.08)] pt-2 md:flex">
          <button
            type="button"
            onClick={() => setOpen(true)}
            aria-label={openLabel}
            title={openLabel}
            className="ui-chip inline-flex items-center gap-1 rounded-md border border-[rgba(255,135,175,0.14)] bg-[rgba(10,18,35,0.7)] px-2 py-1 text-[var(--axon-accent-blue)] transition-colors hover:bg-[rgba(255,135,175,0.1)]"
          >
            {fileIcon}
            <span>{fileCountLabel}</span>
          </button>
        </div>
        <button
          type="button"
          onClick={() => setOpen(true)}
          aria-label={openLabel}
          title={openLabel}
          className="ui-chip fixed bottom-4 left-4 z-40 inline-flex items-center gap-1.5 rounded-full border border-[rgba(255,135,175,0.2)] bg-[rgba(8,15,30,0.9)] px-3 py-2 text-[var(--axon-accent-blue)] shadow-lg backdrop-blur-sm transition-transform active:scale-95 md:hidden"
        >
          {fileIcon}
          <span>Files</span>
          <span className="rounded bg-[rgba(255,135,175,0.18)] px-1 text-[length:var(--text-2xs)] leading-4 text-[var(--axon-text-primary)]">
            {fileCountLabel}
          </span>
        </button>
      </>
    )
  }

  const panel = (
    <div
      ref={panelRef}
      className="flex w-full flex-shrink-0 flex-col border-r border-[rgba(255,135,175,0.08)] md:w-[260px]"
      style={{ background: 'rgba(3, 7, 18, 0.3)' }}
    >
      {/* Header */}
      <div className="flex items-center justify-between border-b border-[rgba(255,135,175,0.08)] px-3 py-2">
        <div className="min-w-0 flex-1">
          <div className="truncate text-[length:var(--text-md)] font-semibold text-[var(--axon-accent-blue)]">
            {domain}
          </div>
          <div className="ui-meta">{files.length} pages</div>
        </div>
        <button
          type="button"
          onClick={() => setOpen(false)}
          aria-label="Collapse file explorer"
          className="ml-2 flex-shrink-0 rounded p-1 text-[var(--axon-text-muted)] transition-colors hover:bg-[rgba(255,135,175,0.06)] hover:text-[var(--axon-accent-blue)]"
          title="Collapse file explorer"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
            className="size-3.5"
          >
            <path d="m15 18-6-6 6-6" />
          </svg>
        </button>
      </div>

      {/* Filter input */}
      <div className="border-b border-[rgba(255,135,175,0.08)] px-2 py-1.5">
        <input
          type="text"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="Filter pages..."
          className="w-full rounded bg-[rgba(255,135,175,0.04)] px-2 py-1.5 text-[length:var(--text-md)] text-[var(--axon-text-secondary)] placeholder-[var(--axon-text-subtle)] outline-none ring-1 ring-[rgba(255,135,175,0.08)] transition-all focus:ring-[rgba(255,135,175,0.2)]"
        />
      </div>

      {/* File list */}
      <div
        className="flex-1 overflow-y-auto overscroll-contain"
        style={{ WebkitOverflowScrolling: 'touch' } as React.CSSProperties}
      >
        {filteredFiles.map((file) => {
          const isActive = file.relative_path === selectedFile
          const name = displayName(file.url, file.relative_path)
          const crumb = breadcrumb(file.url)
          return (
            <div
              key={file.relative_path}
              role="button"
              tabIndex={0}
              onClick={() => handleSelect(file.relative_path)}
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
                    className={`truncate text-[length:var(--text-md)] font-medium leading-[var(--leading-tight)] ${isActive ? 'text-[var(--axon-accent-blue)]' : 'text-[var(--axon-text-secondary)]'}`}
                  >
                    {name}
                  </div>
                  {crumb && (
                    <div className="truncate text-[length:var(--text-xs)] text-[var(--axon-text-subtle)]">
                      {crumb}
                    </div>
                  )}
                </div>
                <div className="mt-0.5 flex flex-shrink-0 items-center gap-1">
                  <span className="text-[length:var(--text-xs)] tabular-nums text-[var(--axon-text-subtle)]">
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
                      className="rounded p-0.5 text-[var(--axon-text-subtle)] transition-colors hover:text-[var(--axon-accent-blue)]"
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
          <div className="px-3 py-4 text-center text-[length:var(--text-md)] text-[var(--axon-text-subtle)]">
            No matches
          </div>
        )}
      </div>
    </div>
  )

  return (
    <>
      {/* Mobile: animated overlay drawer (always mounted, driven by translate) */}
      <div className="contents md:hidden">
        {/* Backdrop */}
        <button
          type="button"
          aria-label="Close file explorer"
          className={`fixed inset-0 z-40 bg-black/60 backdrop-blur-sm transition-opacity duration-300 ${
            open ? 'opacity-100' : 'pointer-events-none opacity-0'
          }`}
          onClick={() => setOpen(false)}
          onKeyDown={(e) => e.key === 'Escape' && setOpen(false)}
        />
        {/* Drawer */}
        <div
          className={`fixed inset-y-0 left-0 z-50 flex w-[85vw] max-w-[320px] flex-col overflow-hidden rounded-r-xl border-r border-[rgba(255,135,175,0.12)] shadow-2xl transition-transform duration-300 ease-out ${
            open ? 'translate-x-0' : '-translate-x-full'
          }`}
          style={{ background: 'rgba(8, 15, 30, 0.97)' }}
        >
          {panel}
        </div>
      </div>

      {/* Desktop: inline sidebar */}
      <div className="hidden md:contents">{panel}</div>
    </>
  )
}
