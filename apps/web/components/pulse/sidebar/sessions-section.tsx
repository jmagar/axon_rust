'use client'

import { Trash2 } from 'lucide-react'
import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useRecentSessions } from '@/hooks/use-recent-sessions'
import { useWsMessageActions } from '@/hooks/use-ws-messages'
import type { SavedPulseSession } from '@/lib/pulse/session-store'
import { deleteSession, loadSavedSessions } from '@/lib/pulse/session-store'

const ACTIVE_SESSION_ID_KEY = 'axon.web.pulse.active-session-id'

type SessionTab = 'conversations' | 'claude'

function formatRelativeTime(ms: number): string {
  const diffMs = Date.now() - ms
  const diffMins = Math.floor(diffMs / 60_000)
  if (diffMins < 1) return 'just now'
  if (diffMins < 60) return `${diffMins}m ago`
  const diffHours = Math.floor(diffMins / 60)
  if (diffHours < 24) return `${diffHours}h ago`
  return `${Math.floor(diffHours / 24)}d ago`
}

function pluralize(count: number, singular: string): string {
  return `${count} ${singular}${count === 1 ? '' : 's'}`
}

// ── Pulse session row ─────────────────────────────────────────────────────────

function PulseSessionRow({
  session,
  isActive,
  onResume,
  onDelete,
}: {
  session: SavedPulseSession
  isActive: boolean
  onResume: (s: SavedPulseSession) => void
  onDelete: (sessionId: string) => void
}) {
  const [confirmDelete, setConfirmDelete] = useState(false)

  return (
    <div
      className={`group relative w-full rounded border transition-colors ${
        isActive
          ? 'border-[rgba(175,215,255,0.3)] bg-[rgba(175,215,255,0.08)]'
          : 'border-[var(--border-subtle)] bg-[rgba(10,18,35,0.45)] hover:border-[var(--border-standard)] hover:bg-[var(--surface-float)]'
      }`}
    >
      <button
        type="button"
        onClick={() => onResume(session)}
        className="w-full px-2 py-1.5 text-left"
        title={isActive ? 'Currently active session' : `Resume: ${session.title}`}
      >
        {isActive && (
          <p className="text-[10px] font-semibold uppercase tracking-wide text-[var(--axon-primary)]">
            Active
          </p>
        )}
        <p className="truncate text-[length:var(--text-xs)] text-[var(--text-secondary)]">
          {session.title}
        </p>
        <p className="text-[10px] text-[var(--text-dim)]">
          {pluralize(session.messageCount, 'message')} &middot;{' '}
          {formatRelativeTime(session.updatedAt)}
        </p>
      </button>
      {!isActive && (
        <div className="absolute right-1 top-1 opacity-0 transition-opacity group-hover:opacity-100">
          {confirmDelete ? (
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => {
                  onDelete(session.sessionId)
                  setConfirmDelete(false)
                }}
                className="rounded border border-[rgba(255,135,175,0.4)] bg-[rgba(255,135,175,0.12)] px-1.5 py-0.5 text-[10px] text-[var(--axon-secondary)] hover:bg-[rgba(255,135,175,0.2)]"
                title="Confirm delete"
              >
                Delete
              </button>
              <button
                type="button"
                onClick={() => setConfirmDelete(false)}
                className="rounded border border-[var(--border-subtle)] px-1.5 py-0.5 text-[10px] text-[var(--text-dim)] hover:border-[var(--border-standard)]"
                title="Cancel"
              >
                No
              </button>
            </div>
          ) : (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                setConfirmDelete(true)
              }}
              className="rounded p-0.5 text-[var(--text-dim)] hover:bg-[rgba(255,135,175,0.12)] hover:text-[var(--axon-secondary)]"
              title="Delete session"
              aria-label={`Delete session: ${session.title}`}
            >
              <Trash2 className="size-3" />
            </button>
          )}
        </div>
      )}
    </div>
  )
}

// ── Main section ──────────────────────────────────────────────────────────────

export function SessionsSection() {
  const router = useRouter()
  const { resumeWorkspaceSession } = useWsMessageActions()
  const { sessions: claudeSessions, isLoading, error, loadSession, reload } = useRecentSessions()
  const [loadingId, setLoadingId] = useState<string | null>(null)
  const [failedId, setFailedId] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [sortMode, setSortMode] = useState<'recent' | 'oldest'>('recent')
  const [activeTab, setActiveTab] = useState<SessionTab>('conversations')
  const [pulseSessions, setPulseSessions] = useState<SavedPulseSession[]>([])
  const [activeSessionId, setActiveSessionId] = useState<string | null>(() => {
    if (typeof window === 'undefined') return null
    try {
      return window.localStorage.getItem(ACTIVE_SESSION_ID_KEY)
    } catch {
      return null
    }
  })

  // Load pulse sessions on mount + listen for updates
  useEffect(() => {
    setPulseSessions(loadSavedSessions())
  }, [])

  useEffect(() => {
    function syncActiveSession() {
      try {
        setActiveSessionId(window.localStorage.getItem(ACTIVE_SESSION_ID_KEY))
      } catch {
        setActiveSessionId(null)
      }
    }
    function onActiveSessionChanged(event: Event) {
      const detail = (event as CustomEvent<{ sessionId?: string | null }>).detail
      setActiveSessionId(detail?.sessionId ?? null)
    }
    function onPulseSessionsUpdated() {
      setPulseSessions(loadSavedSessions())
    }
    window.addEventListener('focus', syncActiveSession)
    window.addEventListener('storage', syncActiveSession)
    window.addEventListener('axon:active-session-changed', onActiveSessionChanged as EventListener)
    window.addEventListener('axon:pulse-sessions-updated', onPulseSessionsUpdated)
    return () => {
      window.removeEventListener('focus', syncActiveSession)
      window.removeEventListener('storage', syncActiveSession)
      window.removeEventListener(
        'axon:active-session-changed',
        onActiveSessionChanged as EventListener,
      )
      window.removeEventListener('axon:pulse-sessions-updated', onPulseSessionsUpdated)
    }
  }, [])

  // ── Pulse sessions: filter + sort ──────────────────────────────────────────

  const filteredPulseSessions = useMemo(() => {
    const q = query.trim().toLowerCase()
    const filtered = !q
      ? pulseSessions
      : pulseSessions.filter(
          (s) =>
            s.title.toLowerCase().includes(q) ||
            s.preview.toLowerCase().includes(q) ||
            s.documentTitle.toLowerCase().includes(q),
        )
    const sorted = [...filtered]
    if (sortMode === 'oldest') {
      sorted.sort((a, b) => a.updatedAt - b.updatedAt)
    } else {
      sorted.sort((a, b) => b.updatedAt - a.updatedAt)
    }
    return sorted
  }, [query, pulseSessions, sortMode])

  // ── Claude sessions: filter + sort ─────────────────────────────────────────

  const filteredClaudeSessions = useMemo(() => {
    const q = query.trim().toLowerCase()
    const filtered = !q
      ? claudeSessions
      : claudeSessions.filter((session) => {
          const preview = (session.preview ?? '').toLowerCase()
          return (
            preview.includes(q) ||
            session.filename.toLowerCase().includes(q) ||
            session.project.toLowerCase().includes(q)
          )
        })
    const sorted = [...filtered]
    if (sortMode === 'oldest') {
      sorted.sort((a, b) => a.mtimeMs - b.mtimeMs)
    } else {
      sorted.sort((a, b) => b.mtimeMs - a.mtimeMs)
    }
    return sorted
  }, [query, claudeSessions, sortMode])

  // ── Handlers ───────────────────────────────────────────────────────────────

  const handleResumePulseSession = useCallback(
    (session: SavedPulseSession) => {
      // If already active, just navigate
      if (session.sessionId === activeSessionId) {
        router.push('/')
        return
      }
      resumeWorkspaceSession(session.sessionId)
      router.push('/')
    },
    [activeSessionId, resumeWorkspaceSession, router],
  )

  const handleDeletePulseSession = useCallback((sessionId: string) => {
    deleteSession(sessionId)
    setPulseSessions(loadSavedSessions())
  }, [])

  async function handleOpenClaudeSession(id: string) {
    if (loadingId) return
    setLoadingId(id)
    setFailedId(null)
    try {
      const ok = await loadSession(id)
      if (!ok) {
        setFailedId(id)
        return
      }
      router.push('/')
    } catch {
      setFailedId(id)
    } finally {
      setLoadingId(null)
    }
  }

  // ── Render ─────────────────────────────────────────────────────────────────

  const pulseCount = pulseSessions.length
  const claudeCount = claudeSessions.length

  if (isLoading && pulseCount === 0) {
    return (
      <div className="px-3 py-4 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
        Loading sessions...
      </div>
    )
  }

  if (pulseCount === 0 && claudeCount === 0 && !activeSessionId) {
    return (
      <div className="px-3 py-6 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
        No sessions yet. Start a conversation to see it here.
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Tab bar */}
      <div className="flex flex-shrink-0 border-b border-[var(--border-subtle)]">
        <button
          type="button"
          onClick={() => setActiveTab('conversations')}
          className={`flex-1 px-2 py-1.5 text-[11px] font-medium transition-colors ${
            activeTab === 'conversations'
              ? 'border-b-2 border-[var(--axon-primary)] text-[var(--axon-primary)]'
              : 'text-[var(--text-dim)] hover:text-[var(--text-secondary)]'
          }`}
        >
          Conversations{pulseCount > 0 ? ` (${pulseCount})` : ''}
        </button>
        <button
          type="button"
          onClick={() => setActiveTab('claude')}
          className={`flex-1 px-2 py-1.5 text-[11px] font-medium transition-colors ${
            activeTab === 'claude'
              ? 'border-b-2 border-[var(--axon-primary)] text-[var(--axon-primary)]'
              : 'text-[var(--text-dim)] hover:text-[var(--text-secondary)]'
          }`}
        >
          Claude CLI{claudeCount > 0 ? ` (${claudeCount})` : ''}
        </button>
      </div>

      {/* Search + sort controls */}
      <div className="flex-shrink-0 space-y-1.5 px-2 py-2">
        <input
          type="text"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search sessions..."
          aria-label="Search sessions"
          className="w-full rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.55)] px-2 py-1 text-[11px] text-[var(--text-secondary)] placeholder:text-[var(--text-dim)] focus:border-[var(--border-standard)] focus:outline-none"
        />
        <div className="flex items-center justify-between gap-2">
          <select
            value={sortMode}
            onChange={(event) => setSortMode(event.target.value as 'recent' | 'oldest')}
            aria-label="Sort sessions"
            className="rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.55)] px-2 py-1 text-[11px] text-[var(--text-secondary)] focus:border-[var(--border-standard)] focus:outline-none"
          >
            <option value="recent">Recent first</option>
            <option value="oldest">Oldest first</option>
          </select>
          <span className="text-[10px] text-[var(--text-dim)]">
            {activeTab === 'conversations'
              ? `${filteredPulseSessions.length} shown`
              : `${filteredClaudeSessions.length} shown`}
          </span>
        </div>
      </div>

      {/* Error banner */}
      {error && activeTab === 'claude' && (
        <div className="mx-2 mb-2 flex-shrink-0 rounded border border-[rgba(255,135,175,0.3)] bg-[rgba(255,135,175,0.08)] px-2 py-1.5 text-[length:var(--text-xs)] text-[var(--axon-secondary)]">
          <p>{error}</p>
          <button
            type="button"
            onClick={() => void reload()}
            className="mt-1 rounded border border-[var(--border-subtle)] px-1.5 py-0.5 text-[10px] text-[var(--text-secondary)] hover:border-[var(--border-standard)]"
          >
            Retry
          </button>
        </div>
      )}

      {/* Session lists */}
      <div className="flex-1 overflow-y-auto px-2 pb-2">
        {activeTab === 'conversations' && (
          <div className="space-y-1">
            {filteredPulseSessions.length === 0 && (
              <p className="px-2 py-3 text-center text-[11px] text-[var(--text-dim)]">
                {query.trim()
                  ? `No conversations match "${query}"`
                  : 'No saved conversations yet. Chat history is saved automatically.'}
              </p>
            )}
            {filteredPulseSessions.map((session) => (
              <PulseSessionRow
                key={session.sessionId}
                session={session}
                isActive={session.sessionId === activeSessionId}
                onResume={handleResumePulseSession}
                onDelete={handleDeletePulseSession}
              />
            ))}
          </div>
        )}

        {activeTab === 'claude' && (
          <div className="space-y-1">
            {isLoading && (
              <p className="px-2 py-3 text-center text-[11px] text-[var(--text-dim)]">Loading...</p>
            )}
            {!isLoading && filteredClaudeSessions.length === 0 && (
              <p className="px-2 py-3 text-center text-[11px] text-[var(--text-dim)]">
                {query.trim() ? `No sessions match "${query}"` : 'No Claude CLI sessions found.'}
              </p>
            )}
            {filteredClaudeSessions.slice(0, 30).map((session) => {
              const isLoadingRow = loadingId === session.id
              const isFailedRow = failedId === session.id
              return (
                <button
                  key={session.id}
                  type="button"
                  onClick={() => void handleOpenClaudeSession(session.id)}
                  disabled={isLoadingRow}
                  className="w-full rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.45)] px-2 py-1.5 text-left transition-colors hover:border-[var(--border-standard)] hover:bg-[var(--surface-float)] disabled:opacity-60"
                  title={session.filename}
                >
                  {session.project !== 'tmp' && (
                    <p className="truncate text-[10px] font-semibold text-[var(--axon-secondary)]">
                      {session.project}
                    </p>
                  )}
                  <p className="truncate text-[length:var(--text-xs)] text-[var(--text-secondary)]">
                    {session.preview ?? session.filename}
                  </p>
                  <p
                    className={`text-[10px] ${
                      isFailedRow ? 'text-[var(--axon-secondary)]' : 'text-[var(--text-dim)]'
                    }`}
                  >
                    {isLoadingRow
                      ? 'Loading...'
                      : isFailedRow
                        ? 'Failed to load'
                        : formatRelativeTime(session.mtimeMs)}
                  </p>
                </button>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}
