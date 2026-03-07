'use client'

import { ChevronDown, ChevronRight, FolderOpen, Network } from 'lucide-react'
import Link from 'next/link'
import { useEffect, useState } from 'react'
import { apiFetch } from '@/lib/api-fetch'

// ---------------------------------------------------------------------------
// Shared card shell
// ---------------------------------------------------------------------------

function Card({
  icon,
  title,
  href,
  children,
  storageKey,
}: {
  icon: React.ReactNode
  title: string
  href?: string
  children: React.ReactNode
  storageKey: string
}) {
  const [collapsed, setCollapsed] = useState(false)

  useEffect(() => {
    try {
      if (window.localStorage.getItem(storageKey) === 'collapsed') setCollapsed(true)
    } catch {
      // Ignore storage errors.
    }
  }, [storageKey])

  function toggleCollapsed() {
    const next = !collapsed
    setCollapsed(next)
    try {
      if (next) {
        window.localStorage.setItem(storageKey, 'collapsed')
      } else {
        window.localStorage.removeItem(storageKey)
      }
    } catch {
      // Ignore storage errors.
    }
  }

  return (
    <div
      className="flex flex-col rounded-xl border"
      style={{
        borderColor: 'var(--border-subtle)',
        background: 'rgba(4,8,20,0.45)',
        minHeight: collapsed ? undefined : '180px',
      }}
    >
      {/* Header */}
      <div
        className="flex items-center justify-between px-3 py-2"
        style={{
          borderColor: 'var(--border-subtle)',
          borderBottom: collapsed ? undefined : '1px solid var(--border-subtle)',
        }}
      >
        <button
          type="button"
          onClick={toggleCollapsed}
          className="flex items-center gap-1.5 transition-opacity hover:opacity-80"
          aria-expanded={!collapsed}
          aria-label={collapsed ? `Expand ${title}` : `Collapse ${title}`}
        >
          <span className="text-[rgba(175,215,255,0.55)] [&>svg]:size-3.5">{icon}</span>
          <span className="text-[10px] font-semibold uppercase tracking-widest text-[rgba(175,215,255,0.4)]">
            {title}
          </span>
          <ChevronDown
            className={`size-3 text-[rgba(175,215,255,0.3)] transition-transform duration-200 ${collapsed ? '-rotate-90' : ''}`}
          />
        </button>
        {href && !collapsed && (
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
      {!collapsed && <div className="flex-1 overflow-hidden p-2">{children}</div>}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Dim helper text
// ---------------------------------------------------------------------------

function Dim({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center py-4 text-[11px] italic text-[var(--text-dim)]">
      {children}
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
    apiFetch('/api/workspace?action=list&path=')
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
          className="flex items-center gap-1.5 rounded px-2 py-1.5 transition-colors hover:bg-[var(--surface-float)]"
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
  status: 'online' | 'offline' | 'unknown'
}

function McpContent() {
  const [servers, setServers] = useState<McpServerEntry[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    Promise.all([
      apiFetch('/api/mcp').then((r) => r.json()) as Promise<{
        mcpServers?: Record<string, { url?: string }>
      }>,
      apiFetch('/api/mcp/status').then((r) => r.json()) as Promise<{
        servers?: Record<string, { status: 'online' | 'offline' | 'unknown' }>
      }>,
    ])
      .then(([cfg, stat]) => {
        const entries: McpServerEntry[] = Object.entries(cfg.mcpServers ?? {})
          .slice(0, 5)
          .map(([name, s]) => ({
            name,
            type: s.url ? 'http' : 'stdio',
            status: stat.servers?.[name]?.status ?? 'unknown',
          }))
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
          href="/settings/mcp"
          className="flex items-center justify-between rounded px-2 py-1.5 transition-colors hover:bg-[var(--surface-float)]"
        >
          <span className="flex min-w-0 items-center gap-1.5">
            <span
              className="size-1.5 shrink-0 rounded-full"
              style={{
                background:
                  s.status === 'online'
                    ? 'rgba(120,220,140,0.9)'
                    : s.status === 'offline'
                      ? 'rgba(255,100,100,0.8)'
                      : 'rgba(180,180,180,0.35)',
              }}
            />
            <span className="truncate text-[11px] text-[rgba(200,220,245,0.7)]">{s.name}</span>
          </span>
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
    <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2">
      <Card
        icon={<FolderOpen />}
        title="Files"
        href="/workspace"
        storageKey="axon.landing.card.files"
      >
        <FilesContent />
      </Card>
      <Card icon={<Network />} title="MCP" href="/settings/mcp" storageKey="axon.landing.card.mcp">
        <McpContent />
      </Card>
    </div>
  )
}
