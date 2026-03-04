'use client'

import { useSearchParams } from 'next/navigation'
import { memo, Suspense, useEffect, useRef, useState } from 'react'
import { PulseEditorPane } from '@/components/pulse/pulse-editor-pane'
import { usePulseAutosave } from '@/hooks/use-pulse-autosave'
import { apiFetch } from '@/lib/api-fetch'

const SaveStatusBadge = memo(function SaveStatusBadge({
  status,
}: {
  status: 'idle' | 'saving' | 'saved' | 'error'
}) {
  if (status === 'idle') return null
  const label = status === 'saving' ? 'Saving…' : status === 'saved' ? 'Saved' : 'Save failed'
  const color =
    status === 'saving'
      ? 'text-[var(--text-dim)]'
      : status === 'saved'
        ? 'text-[var(--accent-green,#4ade80)]'
        : 'text-[var(--accent-red,#f87171)]'
  return (
    <span
      role={status === 'error' ? 'alert' : 'status'}
      aria-live={status === 'error' ? 'assertive' : 'polite'}
      aria-atomic="true"
      className={`text-[11px] tabular-nums ${color}`}
    >
      {label}
    </span>
  )
})

function EditorPageInner() {
  const searchParams = useSearchParams()
  const docParam = searchParams.get('doc')
  const workspaceParam = searchParams.get('workspace')

  const [markdown, setMarkdown] = useState('')
  const [title, setTitle] = useState('Untitled')
  const [docFilename, setDocFilename] = useState<string | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const loadedDocRef = useRef<string | null>(null)

  // Load pulse doc when ?doc= param changes
  useEffect(() => {
    if (!docParam || loadedDocRef.current === docParam) return
    loadedDocRef.current = docParam
    void (async () => {
      try {
        const res = await apiFetch(`/api/pulse/doc?filename=${encodeURIComponent(docParam)}`)
        if (!res.ok) {
          // Do NOT set docFilename — prevents autosave from creating an orphan file.
          setLoadError(res.status === 404 ? 'Document not found' : 'Failed to load document')
          return
        }
        setLoadError(null)
        const data = (await res.json()) as { title?: string; markdown?: string }
        if (data.title) setTitle(data.title)
        if (data.markdown !== undefined) setMarkdown(data.markdown)
        setDocFilename(docParam)
      } catch {
        setLoadError('Failed to load document')
      }
    })()
  }, [docParam])

  // Load workspace file when ?workspace= param changes
  useEffect(() => {
    if (!workspaceParam || loadedDocRef.current === `ws:${workspaceParam}`) return
    loadedDocRef.current = `ws:${workspaceParam}`
    void (async () => {
      try {
        const res = await fetch(
          `/api/workspace?action=read&path=${encodeURIComponent(workspaceParam)}`,
        )
        if (!res.ok) {
          setLoadError(res.status === 404 ? 'File not found' : 'Failed to load file')
          return
        }
        setLoadError(null)
        const data = (await res.json()) as { name?: string; content?: string; type?: string }
        if (data.type === 'binary') {
          setLoadError('Binary files cannot be displayed in the editor')
          return
        }
        setTitle(data.name ?? workspaceParam.split('/').pop() ?? 'Untitled')
        setMarkdown(data.content ?? '')
        // No docFilename — workspace files are viewed in-editor but saved as new pulse docs if modified
      } catch {
        setLoadError('Failed to load file')
      }
    })()
  }, [workspaceParam])

  const { saveStatus, savedFilename } = usePulseAutosave(markdown, title, docFilename)

  // Sync savedFilename → docFilename after first save creates the file
  useEffect(() => {
    if (savedFilename && !docFilename) {
      setDocFilename(savedFilename)
    }
  }, [savedFilename, docFilename])

  return (
    <div className="flex h-screen flex-col bg-[var(--surface-base,#030712)]">
      {/* Header */}
      <div
        className="flex shrink-0 items-center gap-3 border-b border-[var(--border-subtle)] px-4 py-2"
        style={{ backdropFilter: 'blur(8px)', background: 'rgba(10,18,35,0.72)' }}
      >
        <input
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          className="min-w-0 flex-1 bg-transparent text-base font-medium text-[var(--text-primary)] placeholder-[var(--text-dim)] outline-none"
          placeholder="Untitled"
          aria-label="Document title"
        />
        <SaveStatusBadge status={saveStatus} />
      </div>

      {/* Load error banner */}
      {loadError && (
        <div
          role="alert"
          className="shrink-0 border-b border-[var(--accent-red,#f87171)] bg-[rgba(248,113,113,0.08)] px-4 py-2 text-sm text-[var(--accent-red,#f87171)]"
        >
          {loadError} — any edits will be saved as a new document.
        </div>
      )}

      {/* Editor */}
      <div className="min-h-0 flex-1 overflow-hidden">
        <PulseEditorPane
          markdown={markdown}
          onMarkdownChange={setMarkdown}
          scrollStorageKey="axon.web.editor.scroll"
        />
      </div>
    </div>
  )
}

export default function EditorPage() {
  return (
    <Suspense
      fallback={
        <div className="flex h-screen flex-col bg-[var(--surface-base,#030712)]">
          <div className="flex shrink-0 items-center gap-3 border-b border-[var(--border-subtle)] px-4 py-2 opacity-40">
            <div className="h-4 w-32 rounded bg-[var(--text-dim)]" />
          </div>
        </div>
      }
    >
      <EditorPageInner />
    </Suspense>
  )
}
