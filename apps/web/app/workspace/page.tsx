'use client'

import {
  AlertCircle,
  ArrowLeft,
  Check,
  Clock,
  Copy,
  FileText,
  FolderOpen,
  HardDrive,
  Menu,
} from 'lucide-react'
import dynamic from 'next/dynamic'
import Link from 'next/link'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { CodeViewer } from '@/components/workspace/code-viewer'
import { DirBrowser } from '@/components/workspace/dir-browser'
import { type FileEntry, FileTree } from '@/components/workspace/file-tree'
import { WorkspaceBreadcrumb } from '@/components/workspace/workspace-breadcrumb'

const ContentViewer = dynamic(
  () => import('@/components/content-viewer').then((m) => ({ default: m.ContentViewer })),
  {
    ssr: false,
    loading: () => <div className="animate-pulse h-4 rounded bg-[var(--surface-elevated)]" />,
  },
)

interface FileData {
  type: 'text' | 'binary'
  name: string
  ext?: string
  size: number
  modified: string
  content?: string
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString()
}

const RECENTS_KEY = 'axon.web.workspace.recents'

function isValidFileEntry(v: unknown): v is FileEntry {
  return (
    typeof v === 'object' &&
    v !== null &&
    typeof (v as Record<string, unknown>).name === 'string' &&
    ((v as Record<string, unknown>).type === 'file' ||
      (v as Record<string, unknown>).type === 'directory') &&
    typeof (v as Record<string, unknown>).path === 'string'
  )
}

function loadRecents(): FileEntry[] {
  if (typeof window === 'undefined') return []
  try {
    const raw = localStorage.getItem(RECENTS_KEY)
    if (!raw) return []
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.filter(isValidFileEntry)
  } catch {
    return []
  }
}

function saveRecents(entries: FileEntry[]): void {
  try {
    localStorage.setItem(RECENTS_KEY, JSON.stringify(entries))
  } catch {
    // ignore
  }
}

export default function WorkspacePage() {
  const [selectedEntry, setSelectedEntry] = useState<FileEntry | null>(null)
  const [fileData, setFileData] = useState<FileData | null>(null)
  const [dirChildren, setDirChildren] = useState<FileEntry[] | null>(null)
  const [loadingContent, setLoadingContent] = useState(false)
  const [contentError, setContentError] = useState<string | null>(null)
  const [sidebarOpen, setSidebarOpen] = useState(true)
  const [copied, setCopied] = useState(false)
  const [recents, setRecents] = useState<FileEntry[]>([])

  useEffect(() => {
    setRecents(loadRecents())
  }, [])

  const virtualRoot = useMemo<FileEntry[]>(
    () => [
      {
        name: 'Workspace',
        type: 'directory',
        path: '__workspace',
        apiPath: '',
        virtual: true,
        iconType: 'workspace',
      },
      {
        name: 'Docs',
        type: 'directory',
        path: '__docs',
        virtual: true,
        iconType: 'docs',
        preloadedChildren: [],
      },
      {
        name: 'Favorites',
        type: 'directory',
        path: '__favorites',
        virtual: true,
        iconType: 'favorites',
        preloadedChildren: [],
      },
      {
        name: 'Recents',
        type: 'directory',
        path: '__recents',
        virtual: true,
        iconType: 'recents',
        preloadedChildren: recents,
      },
      {
        name: 'Claude',
        type: 'directory',
        path: '__claude',
        apiPath: '__claude',
        virtual: true,
        iconType: 'claude',
      },
    ],
    [recents],
  )

  const handleSelectEntry = useCallback(async (entry: FileEntry) => {
    setSelectedEntry(entry)
    setFileData(null)
    setDirChildren(null)
    setContentError(null)

    if (entry.type === 'directory') {
      if (entry.preloadedChildren !== undefined) {
        setDirChildren(entry.preloadedChildren)
        return
      }
      setLoadingContent(true)
      try {
        const fetchPath = entry.apiPath ?? entry.path
        const res = await fetch(`/api/workspace?action=list&path=${encodeURIComponent(fetchPath)}`)
        if (!res.ok) {
          const err = await res.json()
          setContentError(err.error ?? 'Failed to load directory')
          return
        }
        const data = await res.json()
        setDirChildren(data.items ?? [])
      } catch {
        setContentError('Network error loading directory')
      } finally {
        setLoadingContent(false)
      }
      return
    }

    // File: add to recents, then fetch content
    setRecents((prev) => {
      const deduped = [entry, ...prev.filter((r) => r.path !== entry.path)].slice(0, 10)
      saveRecents(deduped)
      return deduped
    })

    setLoadingContent(true)
    try {
      const res = await fetch(`/api/workspace?action=read&path=${encodeURIComponent(entry.path)}`)
      if (!res.ok) {
        const err = await res.json()
        setContentError(err.error ?? 'Failed to load file')
        return
      }
      setFileData(await res.json())
    } catch {
      setContentError('Network error loading file')
    } finally {
      setLoadingContent(false)
    }
  }, [])

  const isMarkdown = fileData?.ext === '.md' || fileData?.ext === '.mdx'

  const copyPath = useCallback(() => {
    if (!selectedEntry) return
    navigator.clipboard.writeText(selectedEntry.path).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [selectedEntry])

  const sidebarContent = (
    <>
      <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-3 py-2">
        <span className="text-[10px] font-semibold uppercase tracking-widest text-[var(--text-muted)]">
          Files
        </span>
      </div>
      <div className="flex-1 overflow-y-auto overflow-x-hidden py-1 px-1">
        <FileTree
          entries={virtualRoot}
          selectedPath={selectedEntry?.path ?? null}
          onSelect={(entry) => {
            handleSelectEntry(entry)
            if (typeof window !== 'undefined' && window.innerWidth < 640) {
              setSidebarOpen(false)
            }
          }}
        />
      </div>
    </>
  )

  return (
    <div
      className="flex h-screen flex-col overflow-hidden text-[var(--text-primary)]"
      style={{
        background:
          'radial-gradient(ellipse at 14% 10%, rgba(135,175,255,0.08), transparent 34%), radial-gradient(ellipse at 82% 16%, rgba(255,135,175,0.07), transparent 38%), linear-gradient(180deg,#02040b 0%,#030712 60%,#040a14 100%)',
      }}
    >
      {/* Header */}
      <header
        className="flex h-11 items-center gap-3 border-b border-[var(--border-subtle)] px-4"
        style={{ background: 'rgba(10,18,35,0.9)', backdropFilter: 'blur(16px)' }}
      >
        <Link
          href="/"
          className="flex size-7 shrink-0 items-center justify-center rounded border border-[var(--border-subtle)] bg-[var(--surface-float)] text-[var(--text-muted)] transition-colors hover:bg-[var(--surface-elevated)] hover:text-[var(--axon-primary)] focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
        >
          <ArrowLeft className="size-3.5" />
        </Link>
        <div className="flex size-7 shrink-0 items-center justify-center rounded border border-[var(--border-subtle)] bg-[var(--surface-float)]">
          <FolderOpen className="size-3.5 text-[var(--axon-primary)]" />
        </div>
        <div className="flex-1 min-w-0">
          <h1 className="text-sm font-semibold font-display text-[var(--text-primary)] leading-none">
            Workspace
          </h1>
          <p className="mt-0.5 text-[10px] text-[var(--text-muted)] font-mono">
            Browse your workspace files
          </p>
        </div>
        {selectedEntry?.type === 'file' && (
          <button
            type="button"
            onClick={copyPath}
            className="flex items-center gap-1.5 rounded border border-[var(--border-accent)] bg-[rgba(255,135,175,0.07)] px-3 py-1.5 text-xs text-[var(--axon-secondary)] transition-colors hover:bg-[rgba(255,135,175,0.12)] hover:text-[var(--axon-secondary-strong)] focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
          >
            {copied ? (
              <>
                <Check className="size-3" /> Copied
              </>
            ) : (
              <>
                <Copy className="size-3" /> Copy path
              </>
            )}
          </button>
        )}
      </header>

      {/* Body */}
      <div className="flex flex-1 overflow-hidden">
        {/* Mobile sidebar drawer overlay */}
        {sidebarOpen && (
          <div className="fixed inset-0 z-40 sm:hidden">
            <button
              type="button"
              aria-label="Close sidebar"
              className="absolute inset-0 bg-black/60 backdrop-blur-sm"
              onClick={() => setSidebarOpen(false)}
            />
            <aside
              className="absolute inset-y-0 left-0 flex w-[80vw] max-w-[280px] flex-col overflow-hidden border-r border-[var(--border-subtle)]"
              style={{ background: 'rgba(10,18,35,0.97)' }}
            >
              <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-3 py-3">
                <span className="text-[10px] font-semibold uppercase tracking-widest text-[var(--text-muted)]">
                  Files
                </span>
                <button
                  type="button"
                  onClick={() => setSidebarOpen(false)}
                  className="rounded p-1 text-[var(--text-muted)] hover:text-[var(--axon-primary)] focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)]"
                  aria-label="Close sidebar"
                >
                  <svg
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={2}
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    className="size-3.5"
                  >
                    <path d="M18 6 6 18M6 6l12 12" />
                  </svg>
                </button>
              </div>
              <div className="flex-1 overflow-y-auto overflow-x-hidden py-1 px-1">
                <FileTree
                  entries={virtualRoot}
                  selectedPath={selectedEntry?.path ?? null}
                  onSelect={(entry) => {
                    handleSelectEntry(entry)
                    setSidebarOpen(false)
                  }}
                />
              </div>
            </aside>
          </div>
        )}

        {/* Desktop sidebar — inline collapsible */}
        <aside
          className={[
            'hidden sm:flex flex-shrink-0 flex-col border-r border-[var(--border-subtle)] overflow-hidden transition-all duration-300',
            sidebarOpen ? 'sm:w-56' : 'sm:w-0',
          ].join(' ')}
          style={{ background: 'rgba(10,18,35,0.5)' }}
        >
          {sidebarContent}
        </aside>

        {/* Main viewer */}
        <main className="flex flex-1 flex-col overflow-hidden min-w-0">
          {/* Breadcrumb bar */}
          <div className="flex h-11 items-center gap-2 border-b border-[var(--border-subtle)] px-3 sm:gap-3 sm:px-4">
            <button
              type="button"
              onClick={() => setSidebarOpen((v) => !v)}
              className="flex min-h-[44px] min-w-[44px] shrink-0 items-center justify-center text-[var(--text-muted)] transition-colors hover:text-[var(--axon-primary)] focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1 sm:min-h-0 sm:min-w-0"
              title={sidebarOpen ? 'Collapse sidebar' : 'Expand sidebar'}
              aria-label={sidebarOpen ? 'Collapse sidebar' : 'Expand sidebar'}
            >
              <Menu className="size-3.5" />
            </button>
            <WorkspaceBreadcrumb entry={selectedEntry} />
            {fileData && (
              <div className="ml-auto flex items-center gap-2 shrink-0">
                <span className="flex items-center gap-1 text-[10px] text-[var(--text-muted)]">
                  <HardDrive className="size-3" />
                  {formatBytes(fileData.size)}
                </span>
                <span className="hidden items-center gap-1 text-[10px] text-[var(--text-muted)] sm:flex">
                  <Clock className="size-3" />
                  {formatDate(fileData.modified)}
                </span>
              </div>
            )}
          </div>

          {/* Content area */}
          <div className="flex-1 overflow-auto p-4">
            {!selectedEntry && !loadingContent && !contentError && (
              <div className="flex h-full items-center justify-center">
                <div className="text-center">
                  <FolderOpen className="mx-auto mb-3 size-10 text-[var(--text-dim)]" />
                  <p className="text-sm text-[var(--text-muted)]">
                    Select a file to view its contents
                  </p>
                  <p className="mt-1 text-[11px] text-[var(--text-dim)]">
                    <span className="sm:hidden">Tap the folder icon to browse files</span>
                    <span className="hidden sm:inline">Browse the workspace tree on the left</span>
                  </p>
                </div>
              </div>
            )}

            {loadingContent && (
              <div className="flex h-full items-center justify-center">
                <div className="size-6 animate-spin rounded-full border-2 border-[var(--border-subtle)] border-t-[var(--axon-primary)]" />
              </div>
            )}

            {contentError && (
              <div className="flex items-center gap-2 rounded-lg border border-[var(--border-accent)] bg-[rgba(255,135,175,0.05)] px-4 py-3 text-sm text-[var(--axon-secondary)]">
                <AlertCircle className="size-4 shrink-0" />
                {contentError}
              </div>
            )}

            {selectedEntry?.type === 'directory' && dirChildren !== null && !loadingContent && (
              <DirBrowser entry={selectedEntry} items={dirChildren} onSelect={handleSelectEntry} />
            )}

            {fileData?.type === 'binary' && (
              <div className="rounded-lg border border-[var(--border-subtle)] bg-[var(--surface-float)] p-6 text-center">
                <FileText className="mx-auto mb-3 size-8 text-[var(--text-dim)]" />
                <p className="text-sm text-[var(--text-muted)]">Binary file — cannot display</p>
                <p className="mt-1 text-xs text-[var(--text-dim)]">{formatBytes(fileData.size)}</p>
              </div>
            )}

            {fileData?.type === 'text' &&
              fileData.content !== undefined &&
              (isMarkdown ? (
                <div className="prose-invert max-w-none">
                  <ContentViewer markdown={fileData.content} isProcessing={false} />
                </div>
              ) : (
                <div className="h-full">
                  <CodeViewer
                    content={fileData.content}
                    language={fileData.ext?.slice(1)}
                    fileName={fileData.name}
                  />
                </div>
              ))}
          </div>
        </main>
      </div>

      {/* Mobile FAB: shown when sidebar is closed */}
      {!sidebarOpen && (
        <button
          type="button"
          onClick={() => setSidebarOpen(true)}
          className="fixed bottom-[max(1rem,env(safe-area-inset-bottom,1rem))] left-4 z-40 sm:hidden flex items-center gap-1.5 rounded-full border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.9)] px-3 py-2 backdrop-blur-sm shadow-lg transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-primary)] active:scale-95 focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
          aria-label="Open file browser"
        >
          <FolderOpen className="size-3.5" />
          <span className="text-xs font-medium">Files</span>
        </button>
      )}
    </div>
  )
}
