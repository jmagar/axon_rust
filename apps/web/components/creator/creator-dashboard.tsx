'use client'

import { ArrowLeft, BookOpen, Bot, Code2, FileText, Webhook } from 'lucide-react'
import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useState } from 'react'

// ── Types ──────────────────────────────────────────────────────────────────────

type CategoryName = 'skills' | 'agents' | 'commands' | 'hooks'

interface CreatorFile {
  name: string
  path: string
  size: number
}

interface CategoryResult {
  name: CategoryName
  label: string
  files: CreatorFile[]
}

interface ListResponse {
  categories: CategoryResult[]
}

interface ReadResponse {
  name: string
  size: number
  modified: string
  content: string
}

// ── Constants ──────────────────────────────────────────────────────────────────

const CATEGORY_ICONS: Record<CategoryName, React.ComponentType<{ className?: string }>> = {
  skills: BookOpen,
  agents: Bot,
  commands: Code2,
  hooks: Webhook,
}

const CATEGORIES: { name: CategoryName; label: string }[] = [
  { name: 'skills', label: 'Skills' },
  { name: 'agents', label: 'Agents' },
  { name: 'commands', label: 'Commands' },
  { name: 'hooks', label: 'Hooks' },
]

// ── File list panel ────────────────────────────────────────────────────────────

interface FileListProps {
  files: CreatorFile[]
  selectedPath: string | null
  categoryName: CategoryName
  onSelect: (path: string) => void
  loading: boolean
}

function FileListPanel({ files, selectedPath, categoryName, onSelect, loading }: FileListProps) {
  if (loading) {
    return (
      <div className="flex flex-col gap-1 p-2">
        {[1, 2, 3].map((i) => (
          <div
            key={i}
            className="h-8 animate-shimmer rounded-md"
            style={{
              background:
                'linear-gradient(90deg, rgba(135,175,255,0.05) 25%, rgba(135,175,255,0.1) 50%, rgba(135,175,255,0.05) 75%)',
              backgroundSize: '200% 100%',
            }}
          />
        ))}
      </div>
    )
  }

  if (files.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-2 px-4 py-8 text-center">
        <FileText className="size-6 text-[var(--text-dim)]" />
        <p className="text-[11px] text-[var(--text-dim)]">
          No {categoryName} found
          <br />
          in ~/.claude/
        </p>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-0.5 p-2">
      {files.map((file) => {
        const isSelected = file.path === selectedPath
        return (
          <button
            key={file.path}
            type="button"
            onClick={() => onSelect(file.path)}
            className="flex items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-[12px] transition-colors"
            style={{
              background: isSelected ? 'rgba(135,175,255,0.12)' : 'transparent',
              color: isSelected ? 'var(--axon-primary-strong)' : 'var(--text-secondary)',
              borderLeft: isSelected ? '2px solid var(--axon-primary)' : '2px solid transparent',
            }}
          >
            <FileText className="size-3 shrink-0 opacity-60" />
            <span className="truncate font-mono">{file.name}</span>
          </button>
        )
      })}
    </div>
  )
}

// ── Content panel ──────────────────────────────────────────────────────────────

interface ContentPanelProps {
  content: string
  selectedFile: string | null
  isEditing: boolean
  isSaving: boolean
  isLoading: boolean
  onContentChange: (value: string) => void
  onEditToggle: () => void
  onSave: () => void
}

function ContentPanel({
  content,
  selectedFile,
  isEditing,
  isSaving,
  isLoading,
  onContentChange,
  onEditToggle,
  onSave,
}: ContentPanelProps) {
  if (!selectedFile) {
    return (
      <div className="flex flex-1 items-center justify-center gap-3 text-center">
        <div className="flex flex-col items-center gap-2">
          <FileText className="size-8 text-[var(--text-dim)] opacity-40" />
          <p className="text-[12px] text-[var(--text-dim)]">Select a file to preview</p>
        </div>
      </div>
    )
  }

  if (isLoading) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <div className="size-5 animate-spin rounded-full border-2 border-[rgba(135,175,255,0.2)] border-t-[var(--axon-primary)]" />
      </div>
    )
  }

  const fileName = selectedFile.split('/').pop() ?? selectedFile

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Content toolbar */}
      <div
        className="flex shrink-0 items-center justify-between border-b px-4 py-2"
        style={{ borderColor: 'var(--border-subtle)' }}
      >
        <span
          className="text-[11px] font-medium text-[var(--text-secondary)]"
          style={{ fontFamily: 'var(--font-mono)' }}
        >
          {fileName}
        </span>
        <div className="flex items-center gap-2">
          {isEditing && (
            <button
              type="button"
              onClick={onSave}
              disabled={isSaving}
              className="flex items-center gap-1.5 rounded-lg border px-3 py-1 text-[11px] font-semibold transition-colors disabled:opacity-50"
              style={{
                borderColor: 'rgba(130,217,160,0.3)',
                background: 'rgba(130,217,160,0.1)',
                color: 'var(--axon-success)',
              }}
            >
              {isSaving ? 'Saving…' : 'Save'}
            </button>
          )}
          <button
            type="button"
            onClick={onEditToggle}
            className="flex items-center gap-1.5 rounded-lg border px-3 py-1 text-[11px] font-semibold transition-colors"
            style={{
              borderColor: isEditing ? 'var(--border-accent)' : 'var(--border-subtle)',
              background: isEditing ? 'rgba(255,135,175,0.08)' : 'rgba(135,175,255,0.06)',
              color: isEditing ? 'var(--axon-secondary)' : 'var(--axon-primary)',
            }}
          >
            {isEditing ? 'Cancel' : 'Edit'}
          </button>
        </div>
      </div>

      {/* Content area */}
      <div className="flex-1 overflow-auto">
        {isEditing ? (
          <textarea
            value={content}
            onChange={(e) => onContentChange(e.target.value)}
            className="h-full w-full resize-none bg-transparent p-4 font-mono text-[12px] leading-relaxed text-[var(--text-secondary)] outline-none"
            spellCheck={false}
          />
        ) : (
          <pre className="whitespace-pre-wrap break-words p-4 font-mono text-[12px] leading-relaxed text-[var(--text-secondary)]">
            {content}
          </pre>
        )}
      </div>
    </div>
  )
}

// ── Main dashboard ─────────────────────────────────────────────────────────────

export function CreatorDashboard() {
  const router = useRouter()
  const [activeCategory, setActiveCategory] = useState<CategoryName>('skills')
  const [categories, setCategories] = useState<CategoryResult[]>([])
  const [selectedFile, setSelectedFile] = useState<string | null>(null)
  const [content, setContent] = useState('')
  const [isEditing, setIsEditing] = useState(false)
  const [isSaving, setIsSaving] = useState(false)
  const [listLoading, setListLoading] = useState(true)
  const [contentLoading, setContentLoading] = useState(false)
  const [listError, setListError] = useState<string | null>(null)

  // Fetch category list on mount
  const fetchList = useCallback(async (signal?: AbortSignal) => {
    setListLoading(true)
    setListError(null)
    try {
      const res = await fetch('/api/creator?action=list', { signal })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = (await res.json()) as ListResponse
      setCategories(data.categories)
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') return
      setListError(err instanceof Error ? err.message : 'Failed to load')
    } finally {
      setListLoading(false)
    }
  }, [])

  useEffect(() => {
    const ctrl = new AbortController()
    void fetchList(ctrl.signal)
    return () => ctrl.abort()
  }, [fetchList])

  // Fetch file content on selection
  const fetchContent = useCallback(async (filePath: string, signal?: AbortSignal) => {
    setContentLoading(true)
    setContent('')
    setIsEditing(false)
    try {
      const res = await fetch(`/api/creator?action=read&path=${encodeURIComponent(filePath)}`, {
        signal,
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = (await res.json()) as ReadResponse
      setContent(data.content)
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') return
      setContent(`Error loading file: ${err instanceof Error ? err.message : 'Unknown error'}`)
    } finally {
      setContentLoading(false)
    }
  }, [])

  function handleFileSelect(filePath: string) {
    if (filePath === selectedFile) return
    setSelectedFile(filePath)
    const ctrl = new AbortController()
    void fetchContent(filePath, ctrl.signal)
  }

  function handleCategoryChange(cat: CategoryName) {
    setActiveCategory(cat)
    setSelectedFile(null)
    setContent('')
    setIsEditing(false)
  }

  async function handleSave() {
    if (!selectedFile) return
    setIsSaving(true)
    try {
      const res = await fetch('/api/creator', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: selectedFile, content }),
      })
      if (!res.ok) {
        const err = (await res.json()) as { error?: string }
        throw new Error(err.error ?? `HTTP ${res.status}`)
      }
      setIsEditing(false)
    } catch (err) {
      // Surface save error inline without losing edits
      alert(`Save failed: ${err instanceof Error ? err.message : 'Unknown error'}`)
    } finally {
      setIsSaving(false)
    }
  }

  const activeCategoryData = categories.find((c) => c.name === activeCategory)
  const activeFiles = activeCategoryData?.files ?? []

  return (
    <div
      className="flex min-h-dvh flex-col"
      style={{
        background:
          'radial-gradient(ellipse at 14% 10%, rgba(175,215,255,0.08), transparent 34%), radial-gradient(ellipse at 82% 16%, rgba(255,135,175,0.07), transparent 38%), linear-gradient(180deg,#02040b 0%,#030712 60%,#040a14 100%)',
      }}
    >
      {/* Top bar */}
      <header
        className="sticky top-0 z-30 flex shrink-0 items-center gap-3 border-b px-4"
        style={{
          borderColor: 'var(--border-subtle)',
          background: 'rgba(3,7,18,0.9)',
          backdropFilter: 'blur(16px)',
          height: '3.25rem',
        }}
      >
        <button
          type="button"
          onClick={() => router.back()}
          className="flex min-h-[44px] items-center gap-1.5 rounded-md px-2 py-1 text-[12px] font-medium text-[var(--text-dim)] transition-colors hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)] sm:min-h-0"
          aria-label="Go back"
        >
          <ArrowLeft className="size-3.5" />
          Back
        </button>
        <div className="h-4 w-px bg-[var(--border-subtle)]" />
        <div className="flex items-center gap-2">
          <Code2 className="size-3.5 text-[var(--axon-primary-strong)]" />
          <h1 className="text-[14px] font-semibold text-[var(--text-primary)]">Creator</h1>
        </div>
        <div className="flex-1" />

        {/* Category tabs */}
        <nav className="flex items-center gap-1">
          {CATEGORIES.map((cat) => {
            const Icon = CATEGORY_ICONS[cat.name]
            const isActive = activeCategory === cat.name
            const catData = categories.find((c) => c.name === cat.name)
            const count = catData?.files.length ?? 0
            return (
              <button
                key={cat.name}
                type="button"
                onClick={() => handleCategoryChange(cat.name)}
                className="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors"
                style={{
                  color: isActive ? 'var(--axon-primary-strong)' : 'var(--text-dim)',
                  background: isActive ? 'rgba(135,175,255,0.1)' : 'transparent',
                  borderBottom: isActive
                    ? '2px solid var(--axon-primary)'
                    : '2px solid transparent',
                }}
              >
                <Icon className="size-3" />
                {cat.label}
                {count > 0 && (
                  <span
                    className="rounded-full px-1.5 py-0.5 text-[9px] font-semibold"
                    style={{
                      background: isActive ? 'rgba(135,175,255,0.2)' : 'rgba(135,175,255,0.08)',
                      color: isActive ? 'var(--axon-primary)' : 'var(--text-dim)',
                    }}
                  >
                    {count}
                  </span>
                )}
              </button>
            )
          })}
        </nav>
      </header>

      {/* Error state */}
      {listError && (
        <div className="mx-4 mt-4 rounded-xl border border-[rgba(255,80,80,0.2)] bg-[rgba(255,80,80,0.08)] px-4 py-3 text-[13px] text-red-400">
          {listError}
        </div>
      )}

      {/* Main split layout */}
      <main className="flex flex-1 overflow-hidden" style={{ height: 'calc(100dvh - 3.25rem)' }}>
        {/* Left panel — file list */}
        <aside
          className="w-56 shrink-0 overflow-y-auto border-r"
          style={{
            borderColor: 'var(--border-subtle)',
            background: 'rgba(10,18,35,0.4)',
          }}
        >
          <FileListPanel
            files={activeFiles}
            selectedPath={selectedFile}
            categoryName={activeCategory}
            onSelect={handleFileSelect}
            loading={listLoading}
          />
        </aside>

        {/* Right panel — file content */}
        <section
          className="flex flex-1 flex-col overflow-hidden"
          style={{ background: 'rgba(10,18,35,0.2)' }}
        >
          <ContentPanel
            content={content}
            selectedFile={selectedFile}
            isEditing={isEditing}
            isSaving={isSaving}
            isLoading={contentLoading}
            onContentChange={setContent}
            onEditToggle={() => setIsEditing((v) => !v)}
            onSave={handleSave}
          />
        </section>
      </main>
    </div>
  )
}
