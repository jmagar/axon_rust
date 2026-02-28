'use client'

import { ChevronRight, Clock, FolderOpen, Network } from 'lucide-react'
import Link from 'next/link'
import { useEffect, useState } from 'react'
import { type SessionSummary, useRecentSessions } from '@/hooks/use-recent-sessions'

// ---------------------------------------------------------------------------
// Shared card shell
// ---------------------------------------------------------------------------

function Card({
  icon,
  title,
  href,
  children,
}: {
  icon: React.ReactNode
  title: string
  href?: string
  children: React.ReactNode
}) {
  return (
    <div
      className="flex flex-col rounded-xl border"
      style={{
        borderColor: 'rgba(255,135,175,0.08)',
        background: 'rgba(4,8,20,0.45)',
        minHeight: '180px',
      }}
    >
      {/* Header */}
      <div
        className="flex items-center justify-between border-b px-3 py-2"
        style={{ borderColor: 'rgba(255,135,175,0.06)' }}
      >
        <div className="flex items-center gap-1.5">
          <span className="text-[rgba(175,215,255,0.55)] [&>svg]:size-3.5">{icon}</span>
          <span className="text-[10px] font-semibold uppercase tracking-widest text-[rgba(175,215,255,0.4)]">
            {title}
          </span>
        </div>
        {href && (
          <Link
            href={href}
            className="flex items-center gap-0.5 text-[10px] text-[rgba(175,215,255,0.3)] transition-colors hover:text-[rgba(175,215,255,0.7)]"
          >
            View all
            <ChevronRight className="size-3" />
          </Link>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden p-2">{children}</div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Dim helper text
// ---------------------------------------------------------------------------

function Dim({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center py-4 text-[11px] italic text-[rgba(200,210,230,0.2)]">
      {children}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Sessions card
// ---------------------------------------------------------------------------

function formatRelativeTime(mtimeMs: number): string {
  const diff = Math.floor((Date.now() - mtimeMs) / 60_000)
  if (diff < 1) return 'just now'
  if (diff < 60) return `${diff}m ago`
  const h = Math.floor(diff / 60)
  if (h < 24) return `${h}h ago`
  return `${Math.floor(h / 24)}d ago`
}

function SessionRow({
  session,
  onLoad,
}: {
  session: SessionSummary
  onLoad: (id: string) => Promise<boolean>
}) {
  const [loading, setLoading] = useState(false)

  async function handleClick() {
    if (loading) return
    setLoading(true)
    try {
      await onLoad(session.id)
    } finally {
      setLoading(false)
    }
  }

  return (
    <button
      type="button"
      onClick={() => void handleClick()}
      disabled={loading}
      className="flex w-full items-center justify-between rounded px-2 py-1.5 text-left transition-colors disabled:opacity-50 hover:bg-[rgba(255,135,175,0.05)]"
    >
      <div className="min-w-0 flex-1">
        {session.project !== 'tmp' && (
          <span className="block truncate text-[10px] font-semibold text-[rgba(255,135,175,0.7)]">
            {session.project}
          </span>
        )}
        <span className="block truncate text-[11px] text-[rgba(220,230,245,0.8)]">
          {session.preview ??
            (session.filename.length > 28 ? `${session.filename.slice(0, 28)}…` : session.filename)}
        </span>
      </div>
      <span className="ml-2 shrink-0 text-[10px] text-[rgba(175,215,255,0.3)]">
        {loading ? '…' : formatRelativeTime(session.mtimeMs)}
      </span>
    </button>
  )
}

function SessionsContent() {
  const { sessions, isLoading, loadSession } = useRecentSessions()
  if (isLoading) return <Dim>Loading…</Dim>
  if (sessions.length === 0) return <Dim>No recent sessions</Dim>
  return (
    <div className="flex flex-col gap-0.5">
      {sessions.slice(0, 4).map((s) => (
        <SessionRow key={s.id} session={s} onLoad={loadSession} />
      ))}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Files card
// ---------------------------------------------------------------------------

interface FileEntry {
  name: string
  type: 'file' | 'directory'
  path: string
}

function FilesContent() {
  const [entries, setEntries] = useState<FileEntry[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    fetch('/api/workspace?action=list&path=')
      .then((r) => r.json())
      .then((d: { items?: FileEntry[] }) => setEntries(d.items?.slice(0, 5) ?? []))
      .catch(() => setEntries([]))
      .finally(() => setLoading(false))
  }, [])

  if (loading) return <Dim>Loading…</Dim>
  if (entries.length === 0) return <Dim>Workspace empty or unavailable</Dim>

  return (
    <div className="flex flex-col gap-0.5">
      {entries.map((e) => (
        <Link
          key={e.path}
          href="/workspace"
          className="flex items-center gap-1.5 rounded px-2 py-1.5 transition-colors hover:bg-[rgba(175,215,255,0.05)]"
        >
          <span className="text-[rgba(175,215,255,0.4)]">
            {e.type === 'directory' ? (
              <FolderOpen className="size-3 shrink-0" />
            ) : (
              <span className="inline-block size-3 shrink-0" />
            )}
          </span>
          <span className="truncate font-mono text-[11px] text-[rgba(200,220,245,0.7)]">
            {e.name}
          </span>
        </Link>
      ))}
    </div>
  )
}

// ---------------------------------------------------------------------------
// MCP card
// ---------------------------------------------------------------------------

interface McpServerEntry {
  name: string
  type: 'stdio' | 'http'
}

function McpContent() {
  const [servers, setServers] = useState<McpServerEntry[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    fetch('/api/mcp')
      .then((r) => r.json())
      .then((d: { mcpServers?: Record<string, { url?: string }> }) => {
        const entries: McpServerEntry[] = Object.entries(d.mcpServers ?? {})
          .slice(0, 5)
          .map(([name, cfg]) => ({ name, type: cfg.url ? 'http' : 'stdio' }))
        setServers(entries)
      })
      .catch(() => setServers([]))
      .finally(() => setLoading(false))
  }, [])

  if (loading) return <Dim>Loading…</Dim>
  if (servers.length === 0) return <Dim>No MCP servers configured</Dim>

  return (
    <div className="flex flex-col gap-0.5">
      {servers.map((s) => (
        <Link
          key={s.name}
          href="/mcp"
          className="flex items-center justify-between rounded px-2 py-1.5 transition-colors hover:bg-[rgba(175,215,255,0.05)]"
        >
          <span className="truncate text-[11px] text-[rgba(200,220,245,0.7)]">{s.name}</span>
          <span
            className="ml-2 shrink-0 rounded px-1.5 text-[9px] font-semibold uppercase tracking-wider"
            style={{
              background: s.type === 'http' ? 'rgba(175,215,255,0.08)' : 'rgba(255,135,175,0.08)',
              color: s.type === 'http' ? 'rgba(175,215,255,0.55)' : 'rgba(255,135,175,0.55)',
            }}
          >
            {s.type}
          </span>
        </Link>
      ))}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

export function LandingCards() {
  return (
    <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-3">
      <Card icon={<Clock />} title="Sessions">
        <SessionsContent />
      </Card>
      <Card icon={<FolderOpen />} title="Files" href="/workspace">
        <FilesContent />
      </Card>
      <Card icon={<Network />} title="MCP" href="/mcp">
        <McpContent />
      </Card>
    </div>
  )
}
