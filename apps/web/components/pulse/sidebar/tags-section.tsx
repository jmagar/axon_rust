'use client'

import { ChevronDown, ChevronRight } from 'lucide-react'
import { useEffect, useState } from 'react'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { TagDef, TaggedItem } from './types'

const TAG_DEFS_KEY = 'axon.sidebar.tagDefs'
const TAGGED_ITEMS_KEY = 'axon.sidebar.tags'

function loadTagDefs(): TagDef[] {
  try {
    const raw = localStorage.getItem(TAG_DEFS_KEY)
    return raw ? (JSON.parse(raw) as TagDef[]) : []
  } catch {
    return []
  }
}

function loadTaggedItems(): TaggedItem[] {
  try {
    const raw = localStorage.getItem(TAGGED_ITEMS_KEY)
    return raw ? (JSON.parse(raw) as TaggedItem[]) : []
  } catch {
    return []
  }
}

interface TagGroupProps {
  tag: TagDef
  items: TaggedItem[]
}

function TagGroup({ tag, items }: TagGroupProps) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div>
      <button
        type="button"
        onClick={() => setExpanded((e) => !e)}
        className="flex w-full items-center gap-1.5 px-3 py-1.5 text-left hover:bg-[var(--surface-float)] transition-colors"
      >
        {expanded ? (
          <ChevronDown className="size-3 flex-shrink-0 text-[var(--text-dim)]" />
        ) : (
          <ChevronRight className="size-3 flex-shrink-0 text-[var(--text-dim)]" />
        )}
        <span
          className="size-2 flex-shrink-0 rounded-full"
          style={{ backgroundColor: tag.color }}
        />
        <span className="truncate text-[length:var(--text-md)] text-[var(--text-secondary)]">
          {tag.name}
        </span>
        <span className="ml-auto text-[length:var(--text-xs)] tabular-nums text-[var(--text-dim)]">
          {items.length}
        </span>
      </button>
      {expanded && (
        <div className="pl-6">
          {items.map((item) => (
            <a
              key={item.url}
              href={item.url}
              target="_blank"
              rel="noopener noreferrer"
              className="block truncate border-b border-[var(--border-subtle)] px-3 py-1.5 text-[length:var(--text-xs)] text-[var(--text-dim)] hover:text-[var(--axon-primary)] transition-colors"
              title={item.url}
            >
              {item.url}
            </a>
          ))}
        </div>
      )}
    </div>
  )
}

export function TagsSection() {
  const [tagDefs, setTagDefs] = useState<TagDef[]>([])
  const [taggedItems, setTaggedItems] = useState<TaggedItem[]>([])

  useEffect(() => {
    setTagDefs(loadTagDefs())
    setTaggedItems(loadTaggedItems())
  }, [])

  if (tagDefs.length === 0) {
    return (
      <div className="px-3 py-6 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
        No tags yet
      </div>
    )
  }

  return (
    <ScrollArea className="max-h-[30vh]">
      {tagDefs.map((tag) => {
        const items = taggedItems.filter((item) => item.tagIds.includes(tag.id))
        return <TagGroup key={tag.id} tag={tag} items={items} />
      })}
    </ScrollArea>
  )
}
