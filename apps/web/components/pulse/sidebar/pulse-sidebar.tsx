'use client'

import {
  CheckSquare,
  ChevronLeft,
  ChevronRight,
  Clock,
  FileText,
  FolderOpen,
  Layers,
  LayoutTemplate,
  Paintbrush,
  ScrollText,
  Star,
  Tag,
  TerminalSquare,
} from 'lucide-react'
import Link from 'next/link'
import { useCallback, useEffect, useState } from 'react'
import { type FileEntry, FileTree } from '@/components/workspace/file-tree'

const COLLAPSED_KEY = 'axon.sidebar.collapsed'

const NAV_LINKS = [
  { href: '/', label: 'Files', icon: <FileText className="size-4" /> },
  { href: '/workspace', label: 'Starred', icon: <Star className="size-4" /> },
  { href: '/workspace', label: 'Recents', icon: <Clock className="size-4" /> },
  { href: '/workspace', label: 'Tags', icon: <Tag className="size-4" /> },
  { href: '/creator', label: 'Skills', icon: <LayoutTemplate className="size-4" /> },
  { href: '/creator', label: 'Creator', icon: <Paintbrush className="size-4" /> },
  { href: '/tasks', label: 'Tasks', icon: <CheckSquare className="size-4" /> },
  { href: '/jobs', label: 'Jobs', icon: <Layers className="size-4" /> },
  { href: '/logs', label: 'Logs', icon: <ScrollText className="size-4" /> },
  { href: '/terminal', label: 'Terminal', icon: <TerminalSquare className="size-4" /> },
]

export function PulseSidebar() {
  const [collapsed, setCollapsed] = useState(false)
  const [workspaceOpen, setWorkspaceOpen] = useState(false)
  const [workspaceEntries, setWorkspaceEntries] = useState<FileEntry[]>([])
  const [workspaceLoading, setWorkspaceLoading] = useState(false)
  const [selectedPath, setSelectedPath] = useState<string | null>(null)

  useEffect(() => {
    try {
      const stored = localStorage.getItem(COLLAPSED_KEY)
      const next = stored === 'true'
      setCollapsed(next)
      document.documentElement.style.setProperty('--sidebar-w', next ? '48px' : '260px')
    } catch {
      /* ignore */
    }
  }, [])

  const toggleCollapsed = () => {
    setCollapsed((prev) => {
      const next = !prev
      try {
        localStorage.setItem(COLLAPSED_KEY, String(next))
        document.documentElement.style.setProperty('--sidebar-w', next ? '48px' : '260px')
      } catch {
        /* ignore */
      }
      return next
    })
  }

  const toggleWorkspace = useCallback(async () => {
    if (collapsed) {
      setCollapsed(false)
      try {
        localStorage.setItem(COLLAPSED_KEY, 'false')
        document.documentElement.style.setProperty('--sidebar-w', '260px')
      } catch {
        /* ignore */
      }
    }

    const next = !workspaceOpen
    setWorkspaceOpen(next)

    if (next && workspaceEntries.length === 0) {
      setWorkspaceLoading(true)
      try {
        const res = await fetch('/api/workspace?action=list&path=')
        const data: { items?: FileEntry[] } = await res.json()
        setWorkspaceEntries(data.items ?? [])
      } catch {
        /* ignore */
      } finally {
        setWorkspaceLoading(false)
      }
    }
  }, [collapsed, workspaceOpen, workspaceEntries.length])

  const handleSelectFile = useCallback((entry: FileEntry) => {
    setSelectedPath(entry.path)
  }, [])

  const btnCls = `flex items-center gap-2 rounded py-1.5 transition-colors text-[var(--text-muted)] hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)]`
  const btnW = collapsed ? 'w-9 justify-center px-2' : 'w-full px-3'

  return (
    <div
      className={`relative z-[1] flex flex-shrink-0 flex-col border-r border-[var(--border-subtle)] bg-[rgba(10,18,35,0.85)] backdrop-blur-sm transition-all duration-200 ${
        collapsed ? 'w-12' : 'w-[260px]'
      }`}
    >
      {/* Header — AXON logo + collapse toggle */}
      <div
        className={`flex h-11 flex-shrink-0 items-center border-b border-[var(--border-subtle)] px-2 ${
          collapsed ? 'justify-center' : 'justify-between'
        }`}
      >
        {!collapsed && (
          <span
            className="select-none text-sm font-extrabold tracking-[3px]"
            style={{
              background: 'linear-gradient(135deg, #afd7ff 0%, #ff87af 50%, #8787af 100%)',
              WebkitBackgroundClip: 'text',
              WebkitTextFillColor: 'transparent',
              backgroundClip: 'text',
            }}
          >
            AXON
          </span>
        )}
        <button
          type="button"
          onClick={toggleCollapsed}
          aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          className="flex size-6 items-center justify-center rounded border border-[var(--border-subtle)] text-[var(--text-muted)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-primary)]"
        >
          {collapsed ? <ChevronRight className="size-3.5" /> : <ChevronLeft className="size-3.5" />}
        </button>
      </div>

      {/* Nav — page links + workspace inline expander */}
      <nav
        className="flex flex-1 flex-col items-stretch gap-0.5 overflow-y-auto py-2"
        aria-label="Navigation"
      >
        {NAV_LINKS.map((item) => (
          <Link
            key={item.label}
            href={item.href}
            title={item.label}
            aria-label={item.label}
            className={`${btnCls} ${btnW}`}
          >
            {item.icon}
            {!collapsed && (
              <span className="truncate text-[length:var(--text-md)]">{item.label}</span>
            )}
          </Link>
        ))}

        {/* Workspace — toggles inline file tree */}
        <button
          type="button"
          onClick={toggleWorkspace}
          title="Workspace"
          aria-label="Workspace"
          aria-expanded={workspaceOpen}
          className={`${btnCls} ${btnW}`}
        >
          <FolderOpen className="size-4" />
          {!collapsed && <span className="truncate text-[length:var(--text-md)]">Workspace</span>}
        </button>

        {workspaceOpen && !collapsed && (
          <div className="overflow-y-auto">
            {workspaceLoading ? (
              <div className="px-3 py-2 text-xs text-[var(--text-dim)]">Loading...</div>
            ) : (
              <FileTree
                entries={workspaceEntries}
                selectedPath={selectedPath}
                onSelect={handleSelectFile}
              />
            )}
          </div>
        )}
      </nav>
    </div>
  )
}
