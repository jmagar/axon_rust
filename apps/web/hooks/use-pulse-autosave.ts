'use client'

import { useEffect, useRef, useState } from 'react'
import { apiFetch } from '@/lib/api-fetch'

interface DocMeta {
  createdAt: string
  updatedAt: string
  tags: string[]
  collections: string[]
}

export function usePulseAutosave(
  documentMarkdown: string,
  documentTitle: string,
  docFilename?: string | null,
  tabId?: string,
) {
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle')
  const [savedFilename, setSavedFilename] = useState<string | null>(docFilename ?? null)
  const [savedTabId, setSavedTabId] = useState<string | null>(null)
  const filenameRef = useRef<string | null>(docFilename ?? null)
  const tabIdRef = useRef<string | null>(tabId ?? null)
  // Caches createdAt/updatedAt/tags/collections from last save response — sent back on updates
  // so updatePulseDoc can skip the file read and detect concurrent edits.
  const docMetaRef = useRef<DocMeta | null>(null)
  const autosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const autosaveAbortRef = useRef<AbortController | null>(null)
  const idleTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const lastSavedSnapshotRef = useRef('')

  // Keep tabIdRef in sync so the save closure captures the right tab at trigger time
  useEffect(() => {
    tabIdRef.current = tabId ?? null
  }, [tabId])

  // Sync refs when docFilename prop changes (e.g. loading a different file).
  // Only wipe docMetaRef and the snapshot guard when the filename actually changes to a
  // different value — preserves cached metadata when docFilename syncs back the same
  // value that was already set by the first save (savedFilename -> currentDocFilename -> prop).
  useEffect(() => {
    const incoming = docFilename ?? null
    if (incoming !== filenameRef.current) {
      docMetaRef.current = null
      lastSavedSnapshotRef.current = ''
    }
    filenameRef.current = incoming
    setSavedFilename(incoming)
  }, [docFilename])

  // Debounced save effect — 1500ms debounce, POST to /api/pulse/save
  useEffect(() => {
    if (!documentMarkdown || !documentTitle) return

    if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current)
    const snapshot = `${documentTitle}\n---\n${documentMarkdown}`
    if (snapshot === lastSavedSnapshotRef.current) return
    autosaveTimerRef.current = setTimeout(() => {
      // Capture the tab ID at the moment the save fires (not at effect time)
      const capturedTabId = tabIdRef.current
      void (async () => {
        if (autosaveAbortRef.current) {
          autosaveAbortRef.current.abort()
        }
        const controller = new AbortController()
        autosaveAbortRef.current = controller
        try {
          setSaveStatus('saving')
          const body: Record<string, unknown> = {
            title: documentTitle,
            markdown: documentMarkdown,
            embed: true,
          }
          if (filenameRef.current) {
            body.filename = filenameRef.current
            // Include cached metadata so the server can skip the file read and detect conflicts
            if (docMetaRef.current) {
              body.createdAt = docMetaRef.current.createdAt
              body.updatedAt = docMetaRef.current.updatedAt
              body.tags = docMetaRef.current.tags
              body.collections = docMetaRef.current.collections
            }
          }
          const response = await apiFetch('/api/pulse/save', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            signal: controller.signal,
            body: JSON.stringify(body),
          })
          if (response.ok) {
            lastSavedSnapshotRef.current = snapshot
            setSaveStatus('saved')
            const data = (await response.json()) as {
              filename?: string
              createdAt?: string
              updatedAt?: string
              tags?: string[]
              collections?: string[]
            }
            if (data.filename) {
              filenameRef.current = data.filename
              setSavedFilename(data.filename)
              setSavedTabId(capturedTabId)
            }
            // Cache full metadata for next save
            if (data.createdAt && data.updatedAt && data.tags && data.collections) {
              docMetaRef.current = {
                createdAt: data.createdAt,
                updatedAt: data.updatedAt,
                tags: data.tags,
                collections: data.collections,
              }
            }
            // Reset to idle only on success — error state stays visible until next save attempt
            if (idleTimeoutRef.current) clearTimeout(idleTimeoutRef.current)
            idleTimeoutRef.current = setTimeout(() => setSaveStatus('idle'), 2000)
          } else {
            setSaveStatus('error')
          }
        } catch (error: unknown) {
          if (error instanceof Error && error.name === 'AbortError') return
          setSaveStatus('error')
        } finally {
          if (autosaveAbortRef.current === controller) {
            autosaveAbortRef.current = null
          }
        }
      })()
    }, 1500)

    return () => {
      if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current)
    }
  }, [documentMarkdown, documentTitle])

  // Cleanup on unmount — abort both timers and requests
  useEffect(() => {
    return () => {
      if (idleTimeoutRef.current) clearTimeout(idleTimeoutRef.current)
      if (autosaveAbortRef.current) {
        autosaveAbortRef.current.abort()
        autosaveAbortRef.current = null
      }
    }
  }, [])

  return { saveStatus, savedFilename, savedTabId }
}
