'use client'

import { useCallback, useEffect, useRef, useState } from 'react'
import type { SavedPulseSession } from '@/lib/pulse/session-store'
import { deleteSession, loadSavedSessions, saveSession } from '@/lib/pulse/session-store'
import type { ChatMessage } from '@/lib/pulse/workspace-persistence'

export type { SavedPulseSession } from '@/lib/pulse/session-store'

// ── Hook ──────────────────────────────────────────────────────────────────────

interface UsePulseSessionsInput {
  chatSessionId: string | null
  chatHistory: ChatMessage[]
  documentMarkdown: string
  documentTitle: string
}

/**
 * Manages the list of saved Pulse ACP sessions.
 *
 * Auto-saves the current session whenever chatSessionId is set and
 * chatHistory changes (debounced). Provides list, delete, and load
 * operations for the sessions sidebar.
 */
export function usePulseSessions({
  chatSessionId,
  chatHistory,
  documentMarkdown,
  documentTitle,
}: UsePulseSessionsInput) {
  const [sessions, setSessions] = useState<SavedPulseSession[]>([])
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Load sessions on mount
  useEffect(() => {
    setSessions(loadSavedSessions())
  }, [])

  // Auto-save current session (debounced 3s) whenever chat state changes
  // and we have a valid session ID with at least one message.
  useEffect(() => {
    if (!chatSessionId || chatHistory.length === 0) return

    if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    saveTimerRef.current = setTimeout(() => {
      const saved = saveSession(chatSessionId, chatHistory, documentMarkdown, documentTitle)
      if (saved) {
        setSessions(loadSavedSessions())
      }
    }, 3000)

    return () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    }
  }, [chatSessionId, chatHistory, documentMarkdown, documentTitle])

  // Flush on pagehide/visibility hidden (immediate, no debounce)
  useEffect(() => {
    const flush = () => {
      if (!chatSessionId || chatHistory.length === 0) return
      saveSession(chatSessionId, chatHistory, documentMarkdown, documentTitle)
    }
    const onVisibility = () => {
      if (document.visibilityState === 'hidden') flush()
    }
    window.addEventListener('pagehide', flush)
    document.addEventListener('visibilitychange', onVisibility)
    return () => {
      window.removeEventListener('pagehide', flush)
      document.removeEventListener('visibilitychange', onVisibility)
    }
  }, [chatSessionId, chatHistory, documentMarkdown, documentTitle])

  const handleDelete = useCallback((sessionId: string) => {
    deleteSession(sessionId)
    setSessions(loadSavedSessions())
  }, [])

  const reload = useCallback(() => {
    setSessions(loadSavedSessions())
  }, [])

  return {
    savedSessions: sessions,
    deleteSavedSession: handleDelete,
    reloadSavedSessions: reload,
  }
}
