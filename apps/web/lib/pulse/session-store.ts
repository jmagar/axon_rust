/**
 * Pulse session store — localStorage-backed persistence for ACP chat sessions.
 * Pure helpers with zero React imports.
 *
 * Each session snapshot stores enough metadata to display in a list and enough
 * state to fully restore a conversation: session ID, chat history, title,
 * document markdown, timestamps, and message count.
 */

import type { ChatMessage } from '@/lib/pulse/workspace-persistence'

// ── Types ─────────────────────────────────────────────────────────────────────

export interface SavedPulseSession {
  /** ACP session ID returned by the backend */
  sessionId: string
  /** Human-readable title derived from first user message */
  title: string
  /** Preview text (truncated first user message) */
  preview: string
  /** Full chat history for restore */
  chatHistory: ChatMessage[]
  /** Document markdown at time of save */
  documentMarkdown: string
  /** Document title at time of save */
  documentTitle: string
  /** Number of messages in the session */
  messageCount: number
  /** Timestamp when session was first created */
  createdAt: number
  /** Timestamp when session was last updated */
  updatedAt: number
}

// ── Constants ─────────────────────────────────────────────────────────────────

const SESSIONS_KEY = 'axon.web.pulse.saved-sessions'
const MAX_SESSIONS = 50
const MAX_HISTORY_PER_SESSION = 250
const PREVIEW_MAX_LENGTH = 100

// ── Pure helpers ──────────────────────────────────────────────────────────────

function deriveTitle(chatHistory: ChatMessage[]): string {
  const firstUser = chatHistory.find((m) => m.role === 'user')
  if (!firstUser) return 'Untitled conversation'
  const text = firstUser.content.trim().replace(/\n+/g, ' ')
  if (text.length <= 60) return text
  return `${text.slice(0, 60)}...`
}

function derivePreview(chatHistory: ChatMessage[]): string {
  const firstUser = chatHistory.find((m) => m.role === 'user')
  if (!firstUser) return ''
  const text = firstUser.content.trim().replace(/\n+/g, ' ')
  if (text.length <= PREVIEW_MAX_LENGTH) return text
  return `${text.slice(0, PREVIEW_MAX_LENGTH)}...`
}

function readSessionMap(): Map<string, SavedPulseSession> {
  try {
    const raw = window.localStorage.getItem(SESSIONS_KEY)
    if (!raw) return new Map()
    const parsed = JSON.parse(raw) as SavedPulseSession[]
    if (!Array.isArray(parsed)) return new Map()
    const map = new Map<string, SavedPulseSession>()
    for (const s of parsed) {
      if (s && typeof s.sessionId === 'string') {
        map.set(s.sessionId, s)
      }
    }
    return map
  } catch {
    return new Map()
  }
}

function writeSessionMap(map: Map<string, SavedPulseSession>): void {
  try {
    const sorted = Array.from(map.values()).sort((a, b) => b.updatedAt - a.updatedAt)
    const capped = sorted.slice(0, MAX_SESSIONS)
    window.localStorage.setItem(SESSIONS_KEY, JSON.stringify(capped))
  } catch {
    // Ignore quota exceeded or private browsing errors.
  }
}

// ── Public API ────────────────────────────────────────────────────────────────

export function loadSavedSessions(): SavedPulseSession[] {
  const map = readSessionMap()
  return Array.from(map.values()).sort((a, b) => b.updatedAt - a.updatedAt)
}

export function saveSession(
  sessionId: string,
  chatHistory: ChatMessage[],
  documentMarkdown: string,
  documentTitle: string,
): SavedPulseSession | null {
  if (!sessionId || chatHistory.length === 0) return null

  const map = readSessionMap()
  const existing = map.get(sessionId)
  const now = Date.now()

  const session: SavedPulseSession = {
    sessionId,
    title: deriveTitle(chatHistory),
    preview: derivePreview(chatHistory),
    chatHistory: chatHistory.slice(-MAX_HISTORY_PER_SESSION),
    documentMarkdown,
    documentTitle,
    messageCount: chatHistory.length,
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
  }

  map.set(sessionId, session)
  writeSessionMap(map)
  return session
}

export function deleteSession(sessionId: string): boolean {
  const map = readSessionMap()
  const deleted = map.delete(sessionId)
  if (deleted) writeSessionMap(map)
  return deleted
}

export function getSession(sessionId: string): SavedPulseSession | null {
  const map = readSessionMap()
  return map.get(sessionId) ?? null
}
