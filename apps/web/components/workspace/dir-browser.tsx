'use client'

import {
  BookOpen,
  Bot,
  Clock,
  File,
  FileCode,
  FileJson,
  FileText,
  Folder,
  HardDrive,
  Star,
  Tag,
} from 'lucide-react'
import type { FileEntry } from './file-tree'

function dirCardIcon(iconType?: FileEntry['iconType']) {
  switch (iconType) {
    case 'workspace':
      return HardDrive
    case 'docs':
      return BookOpen
    case 'favorites':
      return Star
    case 'recents':
      return Clock
    case 'tags':
      return Tag
    case 'claude':
      return Bot
    default:
      return Folder
  }
}

function fileCardIcon(name: string) {
  const ext = name.split('.').pop()?.toLowerCase() ?? ''
  if (['md', 'mdx', 'txt'].includes(ext)) return FileText
  if (['ts', 'tsx', 'js', 'jsx', 'rs', 'go', 'py', 'sh'].includes(ext)) return FileCode
  if (['json', 'jsonl', 'toml', 'yaml', 'yml'].includes(ext)) return FileJson
  return File
}

function emptyStateMessage(iconType?: FileEntry['iconType']): string {
  switch (iconType) {
    case 'docs':
      return 'Crawled docs will appear here'
    case 'favorites':
      return 'No favorites saved yet'
    case 'recents':
      return 'No recently opened files'
    case 'claude':
      return 'Claude config — skills, commands, agents'
    default:
      return 'This folder is empty'
  }
}

export function DirBrowser({
  entry,
  items,
  onSelect,
}: {
  entry: FileEntry
  items: FileEntry[]
  onSelect: (e: FileEntry) => void
}) {
  if (items.length === 0) {
    const IconComp = dirCardIcon(entry.iconType)
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-center">
          <IconComp className="mx-auto mb-3 size-10 text-[var(--text-dim)]" />
          <p className="text-sm text-[var(--text-muted)]">{emptyStateMessage(entry.iconType)}</p>
        </div>
      </div>
    )
  }

  return (
    <div className="h-full overflow-auto p-4">
      <div className="mb-3 flex items-center gap-2">
        <span className="text-xs font-semibold text-[var(--text-primary)]">{entry.name}</span>
        <span className="text-[10px] text-[var(--text-muted)]">
          {items.length} {items.length === 1 ? 'item' : 'items'}
        </span>
      </div>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
        {items.map((item) => {
          const IconComp =
            item.type === 'directory' ? dirCardIcon(item.iconType) : fileCardIcon(item.name)
          return (
            <button
              key={item.path}
              type="button"
              onClick={() => onSelect(item)}
              className="flex flex-col items-center gap-2 rounded-xl border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.42)] p-3 text-center transition-all hover:border-[rgba(175,215,255,0.2)] hover:bg-[rgba(175,215,255,0.04)] active:scale-95 focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1"
            >
              <IconComp
                className={[
                  'size-7 shrink-0',
                  item.type === 'directory'
                    ? 'text-[rgba(175,215,255,0.55)]'
                    : 'text-[var(--text-dim)]',
                ].join(' ')}
              />
              <span className="w-full truncate font-mono text-[11px] text-[var(--text-secondary)]">
                {item.name}
              </span>
            </button>
          )
        })}
      </div>
    </div>
  )
}
