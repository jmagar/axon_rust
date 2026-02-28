'use client'

import {
  AlertCircle,
  ArrowLeft,
  Check,
  ChevronRight,
  Clock,
  Copy,
  FileText,
  FolderOpen,
  HardDrive,
} from 'lucide-react'
import dynamic from 'next/dynamic'
import Link from 'next/link'
import { useCallback, useEffect, useState } from 'react'
import { CodeViewer } from '@/components/workspace/code-viewer'
import { type FileEntry, FileTree } from '@/components/workspace/file-tree'

const ContentViewer = dynamic(
  () => import('@/components/content-viewer').then((m) => ({ default: m.ContentViewer })),
  {
    ssr: false,
    loading: () => <div className="animate-pulse h-4 bg-[rgba(255,255,255,0.05)] rounded" />,
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

function Breadcrumb({ filePath }: { filePath: string | null }) {
  if (!filePath) {
    return <span className="text-[rgba(175,215,255,0.3)] text-xs font-mono">/workspace</span>
  }
  const parts = filePath.split('/').filter(Boolean)
  return (
    <div className="flex items-center gap-1 font-mono text-xs overflow-x-auto">
      <span className="text-[rgba(175,215,255,0.5)] shrink-0">workspace</span>
      {parts.map((part, i) => {
        const partPath = parts.slice(0, i + 1).join('/')
        const isLast = i === parts.length - 1
        return (
          <span key={partPath} className="flex items-center gap-1 shrink-0">
            <ChevronRight className="size-3 text-[rgba(175,215,255,0.2)]" />
            <span
              className={isLast ? 'text-[rgba(255,135,175,0.8)]' : 'text-[rgba(175,215,255,0.5)]'}
            >
              {part}
            </span>
          </span>
        )
      })}
    </div>
  )
}

export default function WorkspacePage() {
  const [rootEntries, setRootEntries] = useState<FileEntry[]>([])
  const [selectedFile, setSelectedFile] = useState<FileEntry | null>(null)
  const [fileData, setFileData] = useState<FileData | null>(null)
  const [loadingFile, setLoadingFile] = useState(false)
  const [fileError, setFileError] = useState<string | null>(null)
  const [sidebarOpen, setSidebarOpen] = useState(true)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    fetch('/api/workspace?action=list&path=')
      .then((r) => r.json())
      .then((data: { items?: FileEntry[] }) => setRootEntries(data.items ?? []))
      .catch(() => setRootEntries([]))
  }, [])

  const handleSelectFile = useCallback(async (entry: FileEntry) => {
    if (entry.type === 'directory') return
    setSelectedFile(entry)
    setFileData(null)
    setFileError(null)
    setLoadingFile(true)
    try {
      const res = await fetch(`/api/workspace?action=read&path=${encodeURIComponent(entry.path)}`)
      if (!res.ok) {
        const err = await res.json()
        setFileError(err.error ?? 'Failed to load file')
        return
      }
      setFileData(await res.json())
    } catch {
      setFileError('Network error loading file')
    } finally {
      setLoadingFile(false)
    }
  }, [])

  const isMarkdown = fileData?.ext === '.md' || fileData?.ext === '.mdx'

  const copyPath = useCallback(() => {
    if (!selectedFile) return
    navigator.clipboard.writeText(selectedFile.path).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [selectedFile])

  return (
    <div
      className="flex h-screen flex-col overflow-hidden text-white"
      style={{
        background:
          'radial-gradient(ellipse at 14% 10%, rgba(175,215,255,0.08), transparent 34%), radial-gradient(ellipse at 82% 16%, rgba(255,135,175,0.07), transparent 38%), linear-gradient(180deg,#02040b 0%,#030712 60%,#040a14 100%)',
      }}
    >
      {/* Header */}
      <header
        className="flex items-center gap-3 border-b px-4"
        style={{
          minHeight: '52px',
          borderColor: 'rgba(255,135,175,0.1)',
          background: 'rgba(3,7,18,0.9)',
          backdropFilter: 'blur(16px)',
        }}
      >
        <Link
          href="/"
          className="flex size-7 shrink-0 items-center justify-center rounded border border-[rgba(175,215,255,0.12)] bg-[rgba(175,215,255,0.05)] text-[rgba(175,215,255,0.5)] transition-colors hover:bg-[rgba(175,215,255,0.1)] hover:text-[rgba(175,215,255,0.9)]"
        >
          <ArrowLeft className="size-3.5" />
        </Link>

        <div className="flex size-7 shrink-0 items-center justify-center rounded border border-[rgba(175,215,255,0.12)] bg-[rgba(175,215,255,0.05)]">
          <FolderOpen className="size-3.5 text-[rgba(175,215,255,0.7)]" />
        </div>

        <div className="flex-1 min-w-0">
          <h1 className="text-sm font-semibold text-[rgba(200,220,245,0.9)] leading-none">
            Workspace
          </h1>
          <p className="mt-0.5 text-[10px] text-[rgba(175,215,255,0.35)] font-mono">
            Browse your workspace files
          </p>
        </div>

        {selectedFile && (
          <button
            type="button"
            onClick={copyPath}
            className="flex items-center gap-1.5 rounded border border-[rgba(255,135,175,0.15)] bg-[rgba(255,135,175,0.07)] px-3 py-1.5 text-xs text-[rgba(255,135,175,0.8)] transition-colors hover:bg-[rgba(255,135,175,0.12)] hover:text-[rgba(255,135,175,1)]"
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
        {/* Sidebar */}
        <aside
          className={[
            'flex-shrink-0 flex flex-col border-r overflow-hidden transition-all duration-300',
            sidebarOpen ? 'w-64' : 'w-0',
          ].join(' ')}
          style={{ borderColor: 'rgba(175,215,255,0.06)' }}
        >
          <div className="flex items-center justify-between border-b border-[rgba(175,215,255,0.06)] px-3 py-2">
            <span className="text-[10px] font-semibold uppercase tracking-widest text-[rgba(175,215,255,0.3)]">
              Explorer
            </span>
          </div>
          <div className="flex-1 overflow-y-auto overflow-x-hidden py-1 px-1">
            {rootEntries.length === 0 ? (
              <div className="px-3 py-4 text-[11px] text-[rgba(200,210,230,0.25)] italic">
                Loading workspace...
              </div>
            ) : (
              <FileTree
                entries={rootEntries}
                selectedPath={selectedFile?.path ?? null}
                onSelect={handleSelectFile}
              />
            )}
          </div>
        </aside>

        {/* Main viewer */}
        <main className="flex flex-1 flex-col overflow-hidden">
          {/* Breadcrumb bar */}
          <div
            className="flex items-center gap-3 border-b px-4 py-2"
            style={{ borderColor: 'rgba(175,215,255,0.06)', minHeight: '36px' }}
          >
            <button
              type="button"
              onClick={() => setSidebarOpen((v) => !v)}
              className="shrink-0 text-[rgba(175,215,255,0.3)] hover:text-[rgba(175,215,255,0.7)] transition-colors"
              title={sidebarOpen ? 'Collapse sidebar' : 'Expand sidebar'}
            >
              <FolderOpen className="size-3.5" />
            </button>
            <Breadcrumb filePath={selectedFile?.path ?? null} />
            {fileData && (
              <div className="ml-auto flex items-center gap-3 shrink-0">
                <span className="flex items-center gap-1 text-[10px] text-[rgba(175,215,255,0.3)]">
                  <HardDrive className="size-3" />
                  {formatBytes(fileData.size)}
                </span>
                <span className="flex items-center gap-1 text-[10px] text-[rgba(175,215,255,0.3)]">
                  <Clock className="size-3" />
                  {formatDate(fileData.modified)}
                </span>
              </div>
            )}
          </div>

          {/* Content area */}
          <div className="flex-1 overflow-auto p-4">
            {!selectedFile && !loadingFile && !fileError && (
              <div className="flex h-full items-center justify-center">
                <div className="text-center">
                  <FolderOpen className="mx-auto mb-3 size-10 text-[rgba(175,215,255,0.12)]" />
                  <p className="text-sm text-[rgba(200,210,230,0.3)]">
                    Select a file to view its contents
                  </p>
                  <p className="mt-1 text-[11px] text-[rgba(175,215,255,0.2)]">
                    Browse the workspace tree on the left
                  </p>
                </div>
              </div>
            )}

            {loadingFile && (
              <div className="flex h-full items-center justify-center">
                <div className="size-6 animate-spin rounded-full border-2 border-[rgba(175,215,255,0.2)] border-t-[rgba(175,215,255,0.7)]" />
              </div>
            )}

            {fileError && (
              <div className="flex items-center gap-2 rounded-lg border border-[rgba(255,135,175,0.15)] bg-[rgba(255,135,175,0.05)] px-4 py-3 text-sm text-[rgba(255,135,175,0.8)]">
                <AlertCircle className="size-4 shrink-0" />
                {fileError}
              </div>
            )}

            {fileData?.type === 'binary' && (
              <div className="rounded-lg border border-[rgba(175,215,255,0.08)] bg-[rgba(4,10,20,0.6)] p-6 text-center">
                <FileText className="mx-auto mb-3 size-8 text-[rgba(175,215,255,0.2)]" />
                <p className="text-sm text-[rgba(200,210,230,0.5)]">
                  Binary file -- cannot display
                </p>
                <p className="mt-1 text-xs text-[rgba(175,215,255,0.3)]">
                  {formatBytes(fileData.size)}
                </p>
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
    </div>
  )
}
