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
  // Tracks latest tabs without adding `tabs` to the ?doc= effect's dependency array,
  // which would abort in-flight fetches on every tab state change.
  const tabsRef = useRef(tabs)
  useEffect(() => {
    tabsRef.current = tabs
  }, [tabs])

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

  // Load ?doc= URL param into a tab (dedupe by docFilename).
  // `tabs` is intentionally accessed via `tabsRef` instead of being listed in deps —
  // including `tabs` would abort in-flight fetches whenever an unrelated tab update
  // changes the array reference.
  useEffect(() => {
    if (!hydrated || !docParam || loadedDocRef.current === docParam) return
    loadedDocRef.current = docParam
    const existing = tabsRef.current.find((t) => t.docFilename === docParam)
    if (existing) {
      activateTab(existing.id)
      return
    }
    const controller = new AbortController()
    void (async () => {
      try {
        const res = await apiFetch(`/api/pulse/doc?filename=${encodeURIComponent(docParam)}`, {
          signal: controller.signal,
        })
        if (!res.ok) {
          openTab({
            title: `Error: ${docParam}`,
            markdown: `> Could not load \`${docParam}\` (server returned ${res.status})`,
            docFilename: null,
          })
          return
        }
        const data = (await res.json()) as { title?: string; markdown?: string }
        openTab({
          title: data.title ?? 'Untitled',
          markdown: data.markdown ?? '',
          docFilename: docParam,
        })
      } catch (err) {
        if (err instanceof Error && err.name === 'AbortError') return
        openTab({
          title: `Error: ${docParam}`,
          markdown: `> Could not load \`${docParam}\``,
          docFilename: null,
        })
      }
    })()
    return () => controller.abort()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hydrated, docParam, activateTab, openTab])

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

  // Autosave active tab — pass activeTabId so savedTabId tracks which tab was saved
  const { saveStatus, savedFilename, savedTabId } = usePulseAutosave(
    activeTab?.markdown ?? '',
    activeTab?.title ?? '',
    activeTab?.docFilename,
    activeTabId,
  )

  // Persist autosave filename back into the tab that triggered the save (not necessarily active)
  useEffect(() => {
    if (!savedFilename || !savedTabId) return
    const tab = tabs.find((t) => t.id === savedTabId)
    if (tab && !tab.docFilename) {
      updateTab(savedTabId, { docFilename: savedFilename })
    }
  }, [savedFilename, savedTabId, tabs, updateTab])

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
