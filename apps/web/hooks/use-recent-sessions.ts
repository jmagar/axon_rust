'use client'

import { useCallback, useEffect, useState } from 'react'
import { useWsMessageActions } from '@/hooks/use-ws-messages'

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
  messages: ParsedMessage[]
}

const MAX_SESSION_MESSAGES = 50

function buildHandoffPrompt(project: string, messages: ParsedMessage[]): string {
  const capped = messages.slice(-MAX_SESSION_MESSAGES)
  const header = `I'm loading a previous Claude Code session from project: **${project}**. Here is the conversation history:`
  const body = capped.map((m) => `### ${m.role.toUpperCase()}:\n${m.content}`).join('\n\n')
  return `${header}\n\n${body}`
}

export function useRecentSessions() {
  const { submitWorkspacePrompt } = useWsMessageActions()
  const [sessions, setSessions] = useState<SessionSummary[]>([])
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    fetch('/api/sessions/list')
      .then((r) => r.json() as Promise<SessionSummary[]>)
      .then((data) => {
        if (!cancelled) setSessions(data)
      })
      .catch(() => {
        if (!cancelled) setSessions([])
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [])

  const loadSession = useCallback(
    async (id: string): Promise<boolean> => {
      const r = await fetch(`/api/sessions/${id}`)
      if (!r.ok) return false
      const data = (await r.json()) as SessionContentResponse
      if (data.messages.length === 0) return false
      const prompt = buildHandoffPrompt(data.project, data.messages)
      submitWorkspacePrompt(prompt)
      return true
    },
    [submitWorkspacePrompt],
  )

  return { sessions, isLoading, loadSession }
}
