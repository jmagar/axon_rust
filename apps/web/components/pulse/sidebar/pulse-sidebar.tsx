'use client'

import {
  Brain,
  ChevronLeft,
  ChevronRight,
  FileText,
  FolderOpen,
  Layers,
  ScrollText,
  TerminalSquare,
} from 'lucide-react'
import Link from 'next/link'
import { usePathname } from 'next/navigation'
import { useCallback, useEffect, useState } from 'react'
import { useWsExecutionState, useWsMessageActions } from '@/hooks/use-ws-messages'
import type { CrawlFile } from '@/lib/ws-protocol'
import { ExtractedSection } from './extracted-section'
import type { SidebarSectionId } from './types'
import { WorkspaceSection } from './workspace-section'

const COLLAPSED_KEY = 'axon.web.sidebar.collapsed'
const LEGACY_COLLAPSED_KEY = 'axon.sidebar.collapsed'

interface NavItem {
  id: SidebarSectionId
  label: string
  icon: React.ReactNode
}

const NAV_ITEMS: NavItem[] = [
  { id: 'extracted', label: 'Extracted', icon: <FileText className="size-4" /> },
  { id: 'workspace', label: 'Workspace', icon: <FolderOpen className="size-4" /> },
]

const PAGE_LINKS = [
  { href: '/jobs', label: 'Jobs', icon: <Layers className="size-4" /> },
  { href: '/logs', label: 'Logs', icon: <ScrollText className="size-4" /> },
  { href: '/terminal', label: 'Terminal', icon: <TerminalSquare className="size-4" /> },
  { href: '/cortex/status', label: 'Cortex', icon: <Brain className="size-4" /> },
]

function SectionContent({
  activeSection,
  crawlFiles,
  selectedFile,
  onSelectFile,
  jobId,
}: {
  activeSection: SidebarSectionId
  crawlFiles: CrawlFile[]
  selectedFile: string | null
  onSelectFile: (path: string) => void
  jobId?: string | null
}) {
  switch (activeSection) {
    case 'extracted':
      return (
        <ExtractedSection
          files={crawlFiles}
          selectedFile={selectedFile}
          onSelectFile={onSelectFile}
          jobId={jobId}
        />
      )
    case 'workspace':
      return <WorkspaceSection />
    default:
      return null
  }
}

export function PulseSidebar() {
  const { crawlFiles, selectedFile, currentJobId: jobId } = useWsExecutionState()
  const { selectFile: onSelectFile } = useWsMessageActions()
  const [collapsed, setCollapsed] = useState(false)
  const [activeSection, setActiveSection] = useState<SidebarSectionId>('extracted')
  const pathname = usePathname()

  const toggleCollapsed = useCallback(() => {
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
  }, [])

  useEffect(() => {
    try {
      const stored =
        localStorage.getItem(COLLAPSED_KEY) ?? localStorage.getItem(LEGACY_COLLAPSED_KEY)
      if (stored !== null) localStorage.setItem(COLLAPSED_KEY, stored)
      localStorage.removeItem(LEGACY_COLLAPSED_KEY)
      // Auto-collapse on mobile viewports (<768px) if no stored preference
      const isMobile = window.innerWidth < 768
      const next = stored !== null ? stored === 'true' : isMobile
      setCollapsed(next)
      document.documentElement.style.setProperty('--sidebar-w', next ? '48px' : '260px')
    } catch {
      /* ignore */
    }
  }, [])

  useEffect(() => {
    const handler = () => toggleCollapsed()
    document.addEventListener('axon:sidebar:toggle', handler)
    return () => document.removeEventListener('axon:sidebar:toggle', handler)
  }, [toggleCollapsed])

  const handleNavClick = (id: SidebarSectionId) => {
    if (collapsed) {
      setCollapsed(false)
      try {
        localStorage.setItem(COLLAPSED_KEY, 'false')
        document.documentElement.style.setProperty('--sidebar-w', '260px')
      } catch {
        /* ignore */
      }
    }
    setActiveSection(id)
  }

  const activeItem = NAV_ITEMS.find((n) => n.id === activeSection)

  return (
    <div
      className={`relative z-[2] flex flex-shrink-0 flex-col border-r border-[var(--border-subtle)] bg-[rgba(10,18,35,0.85)] backdrop-blur-sm transition-all duration-200 ${
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
          <Link
            href="/"
            className="select-none text-sm font-extrabold tracking-[3px]"
            style={{
              background: 'linear-gradient(135deg, #afd7ff 0%, #ff87af 50%, #8787af 100%)',
              WebkitBackgroundClip: 'text',
              WebkitTextFillColor: 'transparent',
              backgroundClip: 'text',
            }}
          >
            AXON
          </Link>
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

      {/* Nav — section tabs + page links */}
      <nav className="flex flex-col items-center gap-0.5 py-2" aria-label="Sidebar navigation">
        {NAV_ITEMS.map((item) => {
          const isActive = item.id === activeSection
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => handleNavClick(item.id)}
              title={item.label}
              aria-label={item.label}
              aria-pressed={isActive}
              className={`flex items-center gap-2 rounded px-2 py-1.5 transition-colors ${
                collapsed ? 'w-9 justify-center' : 'w-full px-3'
              } ${
                isActive
                  ? 'bg-[rgba(135,175,255,0.12)] text-[var(--axon-primary)]'
                  : 'text-[var(--text-muted)] hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)]'
              }`}
            >
              {item.icon}
              {!collapsed && (
                <span className="truncate text-[length:var(--text-md)]">{item.label}</span>
              )}
            </button>
          )
        })}
        {PAGE_LINKS.map((link) => {
          const isActive =
            link.href === '/cortex/status'
              ? (pathname?.startsWith('/cortex') ?? false)
              : pathname === link.href
          return (
            <Link
              key={link.href}
              href={link.href}
              title={link.label}
              aria-label={link.label}
              aria-current={isActive ? 'page' : undefined}
              className={`flex items-center gap-2 rounded py-1.5 transition-colors ${
                collapsed ? 'w-9 justify-center px-2' : 'w-full px-3'
              } ${
                isActive
                  ? 'bg-[rgba(135,175,255,0.12)] text-[var(--axon-primary)]'
                  : 'text-[var(--text-muted)] hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)]'
              }`}
            >
              {link.icon}
              {!collapsed && (
                <span className="truncate text-[length:var(--text-md)]">{link.label}</span>
              )}
            </Link>
          )
        })}
      </nav>

      {/* Section content (only when expanded) */}
      {!collapsed && (
        <div className="flex flex-1 flex-col overflow-hidden border-t border-[var(--border-subtle)]">
          <div className="flex items-center gap-1.5 px-3 py-1.5">
            <span className="text-[var(--text-dim)]">{activeItem?.icon}</span>
            <span className="text-[length:var(--text-xs)] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
              {activeItem?.label}
            </span>
          </div>
          <div className="flex-1 overflow-hidden">
            <SectionContent
              activeSection={activeSection}
              crawlFiles={crawlFiles}
              selectedFile={selectedFile}
              onSelectFile={onSelectFile}
              jobId={jobId}
            />
          </div>
        </div>
      )}
    </div>
  )
}
