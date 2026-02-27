'use client'

import { useEffect, useRef, useState } from 'react'

export function usePulseAutosave(documentMarkdown: string, documentTitle: string) {
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle')
  const autosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const autosaveAbortRef = useRef<AbortController | null>(null)
  const idleTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const lastSavedSnapshotRef = useRef('')

  // Debounced save effect — 1500ms debounce, POST to /api/pulse/save
  useEffect(() => {
    if (!documentMarkdown || !documentTitle) return

    if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current)
    const snapshot = `${documentTitle}\n---\n${documentMarkdown}`
    if (snapshot === lastSavedSnapshotRef.current) return
    autosaveTimerRef.current = setTimeout(() => {
      void (async () => {
        if (autosaveAbortRef.current) {
          autosaveAbortRef.current.abort()
        }
        const controller = new AbortController()
        autosaveAbortRef.current = controller
        try {
          setSaveStatus('saving')
          const response = await fetch('/api/pulse/save', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            signal: controller.signal,
            body: JSON.stringify({
              title: documentTitle,
              markdown: documentMarkdown,
              embed: true,
            }),
          })
          if (response.ok) {
            lastSavedSnapshotRef.current = snapshot
            setSaveStatus('saved')
          } else {
            setSaveStatus('error')
          }
          if (idleTimeoutRef.current) clearTimeout(idleTimeoutRef.current)
          idleTimeoutRef.current = setTimeout(() => setSaveStatus('idle'), 2000)
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

  return { saveStatus }
}
