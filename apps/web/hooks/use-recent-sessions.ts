'use client'

import { useCallback, useEffect, useState } from 'react'
import { useWsMessageActions } from '@/hooks/use-ws-messages'
import { apiFetch } from '@/lib/api-fetch'

export interface SessionSummary {
  id: string
  project: string
  filename: string
  mtimeMs: number
  sizeBytes: number
  preview?: string
}

interface ParsedMessage {
  role: 'user' | 'assistant'
  content: string
}

interface SessionContentResponse {
  project: string
  filename: string
  sessionId: string
  messages: ParsedMessage[]
}

function dedupeSessions(list: SessionSummary[]): SessionSummary[] {
  const seen = new Map<string, SessionSummary>()
  for (const session of list) {
    const key = session.id
    const existing = seen.get(key)
    if (!existing) {
      seen.set(key, session)
      continue
    }
    if (session.mtimeMs > existing.mtimeMs) {
      seen.set(key, session)
      continue
    }
    if (session.mtimeMs === existing.mtimeMs) {
      if (existing.project === 'tmp' && session.project !== 'tmp') {
        seen.set(key, session)
      }
    }
  }
  return Array.from(seen.values()).sort((a, b) => b.mtimeMs - a.mtimeMs)
}

export function useRecentSessions() {
  const { resumeWorkspaceSession } = useWsMessageActions()
  const [sessions, setSessions] = useState<SessionSummary[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const reload = useCallback(async () => {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), 8_000)
    setIsLoading(true)
    setError(null)
    try {
      const response = await apiFetch('/api/sessions/list', {
        signal: controller.signal,
      })
      if (!response.ok) {
        setSessions([])
        setError(`Failed to load sessions (${response.status})`)
        return
      }
      const data = (await response.json()) as SessionSummary[]
      setSessions(Array.isArray(data) ? dedupeSessions(data) : [])
    } catch {
      setSessions([])
      setError('Failed to load sessions')
    } finally {
      clearTimeout(timeout)
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    void reload()
  }, [reload])

  const loadSession = useCallback(
    async (id: string): Promise<boolean> => {
      const r = await apiFetch(`/api/sessions/${id}`)
      if (!r.ok) return false
      const data = (await r.json()) as SessionContentResponse
      if (!data.sessionId) return false
      resumeWorkspaceSession(data.sessionId)
      return true
    },
    [resumeWorkspaceSession],
  )

  return { sessions, isLoading, error, loadSession, reload }
}
