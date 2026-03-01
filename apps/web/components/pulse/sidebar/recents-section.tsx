'use client'

import { ExternalLink } from 'lucide-react'
import Link from 'next/link'
import { useEffect, useState } from 'react'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { RecentItem } from './types'

const STORAGE_KEY = 'axon.sidebar.recents'
const MAX_RECENTS = 20

function loadRecents(): RecentItem[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    return JSON.parse(raw) as RecentItem[]
  } catch {
    return []
  }
}

export function pushRecent(url: string, title: string): void {
  try {
    const items = loadRecents().filter((i) => i.url !== url)
    const next: RecentItem[] = [{ url, title, accessedAt: Date.now() }, ...items].slice(
      0,
      MAX_RECENTS,
    )
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
  } catch {
    /* ignore */
  }
}

function formatRelativeTime(ts: number): string {
  const diffMs = Date.now() - ts
  const diffMins = Math.floor(diffMs / 60_000)
  if (diffMins < 1) return 'just now'
  if (diffMins < 60) return `${diffMins}m ago`
  const diffHours = Math.floor(diffMins / 60)
  if (diffHours < 24) return `${diffHours}h ago`
  return `${Math.floor(diffHours / 24)}d ago`
}

export function RecentsSection() {
  const [items, setItems] = useState<RecentItem[]>([])

  useEffect(() => {
    setItems(loadRecents())
  }, [])

  if (items.length === 0) {
    return (
      <div className="px-3 py-6 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
        No recent items
      </div>
    )
  }

  return (
    <ScrollArea className="h-full">
      {items.map((item) => {
        const isExternal = /^https?:\/\//.test(item.url)
        const rowContent = (
          <>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[length:var(--text-md)] font-medium text-[var(--text-secondary)] group-hover:text-[var(--text-primary)]">
                {item.title}
              </div>
              <div className="truncate text-[length:var(--text-xs)] text-[var(--text-dim)]">
                {formatRelativeTime(item.accessedAt)}
              </div>
            </div>
            {isExternal && (
              <ExternalLink className="size-3 flex-shrink-0 text-[var(--text-dim)] opacity-0 transition-opacity group-hover:opacity-100" />
            )}
          </>
        )

        return isExternal ? (
          <a
            key={item.url}
            href={item.url}
            target="_blank"
            rel="noopener noreferrer"
            title={item.title}
            className="group flex items-center justify-between gap-1.5 border-b border-[var(--border-subtle)] px-3 py-2 hover:bg-[var(--surface-float)]"
          >
            {rowContent}
          </a>
        ) : (
          <Link
            key={item.url}
            href={item.url}
            title={item.title}
            className="group flex items-center justify-between gap-1.5 border-b border-[var(--border-subtle)] px-3 py-2 hover:bg-[var(--surface-float)]"
          >
            {rowContent}
          </Link>
        )
      })}
    </ScrollArea>
  )
}
