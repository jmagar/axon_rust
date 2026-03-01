'use client'

import { useSearchParams } from 'next/navigation'
import { Suspense, useEffect, useRef, useState } from 'react'
import { PulseEditorPane } from '@/components/pulse/pulse-editor-pane'
import { usePulseAutosave } from '@/hooks/use-pulse-autosave'

function SaveStatusBadge({ status }: { status: 'idle' | 'saving' | 'saved' | 'error' }) {
  if (status === 'idle') return null
  const label = status === 'saving' ? 'Saving…' : status === 'saved' ? 'Saved' : 'Save failed'
  const color =
    status === 'saving'
      ? 'text-[var(--text-dim)]'
      : status === 'saved'
        ? 'text-[var(--accent-green,#4ade80)]'
        : 'text-[var(--accent-red,#f87171)]'
  return <span className={`text-[11px] tabular-nums ${color}`}>{label}</span>
}

function EditorPageInner() {
  const searchParams = useSearchParams()
  const docParam = searchParams.get('doc')

  const [markdown, setMarkdown] = useState('')
  const [title, setTitle] = useState('Untitled')
  const [docFilename, setDocFilename] = useState<string | null>(null)
  const loadedRef = useRef(false)

  // Load doc from ?doc= param on mount
  useEffect(() => {
    if (!docParam || loadedRef.current) return
    loadedRef.current = true
    void (async () => {
      try {
        const res = await fetch(`/api/pulse/doc?filename=${encodeURIComponent(docParam)}`)
        if (!res.ok) return
        const data = (await res.json()) as { title?: string; markdown?: string }
        if (data.title) setTitle(data.title)
        if (data.markdown !== undefined) setMarkdown(data.markdown)
        setDocFilename(docParam)
      } catch {
        // Silently fail — user can start fresh
      }
    })()
  }, [docParam])

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
    <Suspense>
      <EditorPageInner />
    </Suspense>
  )
}
