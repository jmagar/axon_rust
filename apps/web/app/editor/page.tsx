'use client'

import { useSearchParams } from 'next/navigation'
import { Suspense, useEffect, useRef } from 'react'
import { EditorTabBar } from '@/components/editor-tab-bar'
import { PulseEditorPane } from '@/components/pulse/pulse-editor-pane'
import { usePulseAutosave } from '@/hooks/use-pulse-autosave'
import { useTabs } from '@/hooks/use-tabs'
import { apiFetch } from '@/lib/api-fetch'
import { consumePendingTab, onPendingTab } from '@/lib/pending-tab'

function EditorPageInner() {
  const searchParams = useSearchParams()
  const docParam = searchParams.get('doc')
  const workspaceParam = searchParams.get('workspace')

  const { tabs, activeTabId, activeTab, hydrated, openTab, closeTab, activateTab, updateTab } =
    useTabs()

  const loadedDocRef = useRef<string | null>(null)
  const loadedWorkspaceRef = useRef<string | null>(null)

  // Consume pending tab written by Cmd+K background execution on page load
  useEffect(() => {
    if (!hydrated) return
    const pending = consumePendingTab()
    if (pending) {
      openTab({ title: pending.title, markdown: pending.markdown, docFilename: null })
    }
  }, [hydrated, openTab])

  // Listen for pending tabs arriving while this page is already open
  useEffect(() => {
    return onPendingTab((tab) => {
      openTab({ title: tab.title, markdown: tab.markdown, docFilename: null })
    })
  }, [openTab])

  // Load ?doc= URL param into a tab (dedupe by docFilename)
  useEffect(() => {
    if (!hydrated || !docParam || loadedDocRef.current === docParam) return
    loadedDocRef.current = docParam
    const existing = tabs.find((t) => t.docFilename === docParam)
    if (existing) {
      activateTab(existing.id)
      return
    }
    void (async () => {
      try {
        const res = await apiFetch(`/api/pulse/doc?filename=${encodeURIComponent(docParam)}`)
        if (!res.ok) return
        const data = (await res.json()) as { title?: string; markdown?: string }
        openTab({
          title: data.title ?? 'Untitled',
          markdown: data.markdown ?? '',
          docFilename: docParam,
        })
      } catch {
        // ignore — tab simply won't open
      }
    })()
  }, [hydrated, docParam, tabs, activateTab, openTab])

  // Load ?workspace= URL param into a tab
  useEffect(() => {
    if (!hydrated || !workspaceParam || loadedWorkspaceRef.current === workspaceParam) return
    loadedWorkspaceRef.current = workspaceParam
    void (async () => {
      try {
        const res = await fetch(
          `/api/workspace?action=read&path=${encodeURIComponent(workspaceParam)}`,
        )
        if (!res.ok) return
        const data = (await res.json()) as { name?: string; content?: string; type?: string }
        if (data.type === 'binary') return
        openTab({
          title: data.name ?? workspaceParam.split('/').pop() ?? 'Untitled',
          markdown: data.content ?? '',
          docFilename: null,
        })
      } catch {
        // ignore
      }
    })()
  }, [hydrated, workspaceParam, openTab])

  // Autosave active tab
  const { saveStatus, savedFilename } = usePulseAutosave(
    activeTab?.markdown ?? '',
    activeTab?.title ?? '',
    activeTab?.docFilename,
  )

  // Persist autosave filename back into tab once created
  useEffect(() => {
    if (savedFilename && activeTab && !activeTab.docFilename) {
      updateTab(activeTabId, { docFilename: savedFilename })
    }
  }, [savedFilename, activeTab, activeTabId, updateTab])

  if (!hydrated) {
    return <div className="flex h-screen bg-[var(--surface-base,#030712)]" />
  }

  return (
    <div className="flex h-screen flex-col bg-[var(--surface-base,#030712)]">
      <EditorTabBar
        tabs={tabs}
        activeTabId={activeTabId}
        saveStatus={saveStatus}
        onActivate={activateTab}
        onClose={closeTab}
        onNewTab={() => openTab({ title: 'Untitled', markdown: '', docFilename: null })}
      />

      {/* Title bar */}
      <div
        className="flex shrink-0 items-center gap-3 border-b border-[var(--border-subtle)] px-4 py-2"
        style={{ backdropFilter: 'blur(8px)', background: 'rgba(10,18,35,0.72)' }}
      >
        <input
          type="text"
          value={activeTab?.title ?? ''}
          onChange={(e) => updateTab(activeTabId, { title: e.target.value })}
          className="min-w-0 flex-1 bg-transparent text-base font-medium text-[var(--text-primary)] placeholder-[var(--text-dim)] outline-none"
          placeholder="Untitled"
          aria-label="Document title"
        />
        {saveStatus === 'saving' && (
          <span
            role="status"
            aria-live="polite"
            className="text-[11px] tabular-nums text-[var(--text-dim)]"
          >
            Saving…
          </span>
        )}
        {saveStatus === 'saved' && (
          <span
            role="status"
            aria-live="polite"
            className="text-[11px] tabular-nums text-[var(--accent-green,#4ade80)]"
          >
            Saved
          </span>
        )}
        {saveStatus === 'error' && (
          <span
            role="alert"
            aria-live="assertive"
            aria-atomic="true"
            className="text-[11px] tabular-nums text-[var(--accent-red,#f87171)]"
          >
            Save failed
          </span>
        )}
      </div>

      {/* Editor — keyed on activeTabId so each tab gets its own Plate instance */}
      <div className="min-h-0 flex-1 overflow-hidden">
        {activeTab && (
          <PulseEditorPane
            key={activeTabId}
            markdown={activeTab.markdown}
            onMarkdownChange={(md) => updateTab(activeTabId, { markdown: md })}
            scrollStorageKey={`axon.web.editor.scroll.${activeTabId}`}
          />
        )}
      </div>
    </div>
  )
}

export default function EditorPage() {
  return (
    <Suspense fallback={<div className="flex h-screen bg-[var(--surface-base,#030712)]" />}>
      <EditorPageInner />
    </Suspense>
  )
}
