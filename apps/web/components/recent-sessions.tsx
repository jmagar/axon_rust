'use client'

import { useState } from 'react'
import { type SessionSummary, useRecentSessions } from '@/hooks/use-recent-sessions'

function formatRelativeTime(mtimeMs: number): string {
  const diffMs = Date.now() - mtimeMs
  const diffMins = Math.floor(diffMs / 60_000)
  if (diffMins < 1) return 'just now'
  if (diffMins < 60) return `${diffMins}m ago`
  const diffHours = Math.floor(diffMins / 60)
  if (diffHours < 24) return `${diffHours}h ago`
  return `${Math.floor(diffHours / 24)}d ago`
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`
  return `${(bytes / 1024).toFixed(0)}KB`
}

function SessionCard({
  session,
  onLoad,
}: {
  session: SessionSummary
  onLoad: (id: string) => Promise<boolean>
}) {
  const [loading, setLoading] = useState(false)
  const [failed, setFailed] = useState(false)

  async function handleClick() {
    if (loading) return
    setLoading(true)
    setFailed(false)
    try {
      const ok = await onLoad(session.id)
      if (!ok) setFailed(true)
    } catch {
      setFailed(true)
    } finally {
      setLoading(false)
    }
  }

  return (
    <button
      type="button"
      onClick={() => void handleClick()}
      disabled={loading}
      className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left transition-colors disabled:opacity-50"
      style={{
        border: '1px solid var(--axon-border)',
        background: loading ? 'rgba(255,135,175,0.04)' : 'transparent',
      }}
      onMouseEnter={(e) => {
        if (!loading) e.currentTarget.style.background = 'rgba(255,135,175,0.06)'
      }}
      onMouseLeave={(e) => {
        if (!loading) e.currentTarget.style.background = 'transparent'
      }}
    >
      <div className="min-w-0 flex-1">
        {session.project !== 'tmp' && (
          <span
            className="block truncate text-xs font-semibold"
            style={{ color: 'var(--axon-accent-pink)' }}
          >
            {session.project}
          </span>
        )}
        <span className="block truncate text-[11px]" style={{ color: 'white' }}>
          {session.preview ??
            (session.filename.length > 20 ? `${session.filename.slice(0, 20)}…` : session.filename)}
        </span>
      </div>
      <div className="ml-3 shrink-0 text-right">
        <span
          className="block text-[11px]"
          style={{ color: failed ? 'var(--axon-accent-blue)' : 'var(--axon-text-subtle)' }}
        >
          {loading ? 'Loading…' : failed ? 'Failed to load' : formatRelativeTime(session.mtimeMs)}
        </span>
        <span className="block text-[10px]" style={{ color: 'var(--axon-text-dim)' }}>
          {formatBytes(session.sizeBytes)}
        </span>
      </div>
    </button>
  )
}

export function RecentSessions() {
  const { sessions, isLoading, loadSession } = useRecentSessions()

  if (isLoading) {
    return (
      <div className="mt-3 text-center text-xs" style={{ color: 'var(--axon-text-dim)' }}>
        Loading sessions…
      </div>
    )
  }

  if (sessions.length === 0) return null

  return (
    <div className="mt-3">
      <div
        className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-wider"
        style={{ color: '#ff87af' }}
      >
        Recent Sessions
      </div>
      <div className="flex flex-col gap-1">
        {sessions.slice(0, 4).map((session) => (
          <SessionCard key={session.id} session={session} onLoad={loadSession} />
        ))}
      </div>
    </div>
  )
}
