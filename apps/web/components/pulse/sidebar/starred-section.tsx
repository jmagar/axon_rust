'use client'

import { ExternalLink, Star } from 'lucide-react'
import { useEffect, useState } from 'react'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { StarredItem } from './types'

const STORAGE_KEY = 'axon.sidebar.starred'

function loadStarred(): StarredItem[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    return JSON.parse(raw) as StarredItem[]
  } catch {
    return []
  }
}

export function saveStarred(items: StarredItem[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items))
  } catch {
    /* ignore */
  }
}

export function addStarred(url: string, title: string): void {
  const items = loadStarred()
  const exists = items.some((i) => i.url === url)
  if (exists) return
  saveStarred([{ url, title, starredAt: Date.now() }, ...items])
}

export function removeStarred(url: string): void {
  saveStarred(loadStarred().filter((i) => i.url !== url))
}

export function StarredSection() {
  const [items, setItems] = useState<StarredItem[]>([])

  useEffect(() => {
    setItems(loadStarred())
  }, [])

  const handleUnstar = (url: string) => {
    removeStarred(url)
    setItems((prev) => prev.filter((i) => i.url !== url))
  }

  if (items.length === 0) {
    return (
      <div className="px-3 py-6 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
        No starred items yet
      </div>
    )
  }

  return (
    <ScrollArea className="max-h-[30vh]">
      {items.map((item) => (
        <div
          key={item.url}
          className="group flex items-center justify-between gap-1.5 border-b border-[var(--border-subtle)] px-3 py-2 hover:bg-[var(--surface-float)]"
        >
          <div className="min-w-0 flex-1">
            <div className="truncate text-[length:var(--text-md)] font-medium text-[var(--text-secondary)] group-hover:text-[var(--text-primary)]">
              {item.title}
            </div>
            <div className="truncate text-[length:var(--text-xs)] text-[var(--text-dim)]">
              {item.url}
            </div>
          </div>
          <div className="flex flex-shrink-0 items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
            <a
              href={item.url}
              target="_blank"
              rel="noopener noreferrer"
              title="Open in browser"
              className="rounded p-0.5 text-[var(--text-dim)] hover:text-[var(--axon-primary)]"
            >
              <ExternalLink className="size-3" />
            </a>
            <button
              type="button"
              title="Remove star"
              onClick={() => handleUnstar(item.url)}
              className="rounded p-0.5 text-[var(--axon-secondary)] hover:text-[var(--text-dim)]"
            >
              <Star className="size-3 fill-current" />
            </button>
          </div>
        </div>
      ))}
    </ScrollArea>
  )
}
