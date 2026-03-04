'use client'

import {
  BookOpen,
  Bot,
  ChevronDown,
  ChevronRight,
  Clock,
  File,
  FileCode,
  FileJson,
  FileText,
  Folder,
  FolderOpen,
  HardDrive,
  Star,
  Tag,
} from 'lucide-react'
import { useCallback, useState } from 'react'
import { apiFetch } from '@/lib/api-fetch'

export interface FileEntry {
  name: string
  type: 'file' | 'directory'
  path: string // relative to workspace root
  apiPath?: string
  virtual?: boolean
  iconType?: 'workspace' | 'docs' | 'favorites' | 'recents' | 'tags' | 'claude'
  preloadedChildren?: FileEntry[]
}

interface FileTreeProps {
  entries: FileEntry[]
  selectedPath?: string | null
  onSelect: (entry: FileEntry) => void
  depth?: number
}

function fileIcon(name: string) {
  const ext = name.split('.').pop()?.toLowerCase() ?? ''
  if (['md', 'mdx', 'txt'].includes(ext)) return FileText
  if (['ts', 'tsx', 'js', 'jsx', 'rs', 'go', 'py', 'sh'].includes(ext)) return FileCode
  if (['json', 'jsonl', 'toml', 'yaml', 'yml'].includes(ext)) return FileJson
  return File
}

function dirIcon(entry: FileEntry, expanded: boolean) {
  if (entry.type !== 'directory') return fileIcon(entry.name)
  switch (entry.iconType) {
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
      return expanded ? FolderOpen : Folder
  }
}

function TreeNode({
  entry,
  selectedPath,
  onSelect,
  depth,
}: {
  entry: FileEntry
  selectedPath: string | null
  onSelect: (e: FileEntry) => void
  depth: number
}) {
  const [expanded, setExpanded] = useState(false)
  const [children, setChildren] = useState<FileEntry[] | null>(null)
  const [loading, setLoading] = useState(false)
  const isSelected = selectedPath === entry.path

  const toggle = useCallback(async () => {
    // Always call onSelect for both files and directories
    onSelect(entry)

    if (entry.type !== 'directory') return

    // If preloadedChildren are available, use them directly without fetching
    if (entry.preloadedChildren !== undefined) {
      if (children === null) {
        setChildren(entry.preloadedChildren)
      }
      setExpanded((e) => !e)
      return
    }

    if (!expanded && children === null) {
      setLoading(true)
      try {
        const fetchPath = entry.apiPath ?? entry.path
        const res = await apiFetch(
          `/api/workspace?action=list&path=${encodeURIComponent(fetchPath)}`,
        )
        const data = await res.json()
        setChildren(data.items ?? [])
      } catch {
        setChildren([])
      } finally {
        setLoading(false)
      }
    }
    setExpanded((e) => !e)
  }, [entry, expanded, children, onSelect])

  const indent = depth * 12
  const isVirtualRoot = entry.virtual && depth === 0
  const IconComp = dirIcon(entry, expanded)

  const dirIconColor = isSelected ? 'text-[var(--axon-primary)]' : 'text-[rgba(175,215,255,0.5)]'

  return (
    <div>
      <button
        type="button"
        onClick={toggle}
        className={[
          'flex w-full min-h-[44px] items-center gap-1.5 rounded px-2 py-2 text-left',
          isVirtualRoot ? 'sm:min-h-0 sm:py-1.5' : 'sm:min-h-0 sm:py-[3px]',
          isVirtualRoot ? 'text-[11px] font-medium tracking-wide font-mono' : 'text-xs font-mono',
          'transition-colors duration-150',
          'focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1',
          isSelected
            ? 'bg-[rgba(175,215,255,0.08)] text-[var(--axon-primary)]'
            : 'text-[var(--text-secondary)] hover:bg-[rgba(175,215,255,0.04)] hover:text-[var(--text-primary)]',
        ].join(' ')}
        style={{ paddingLeft: `${8 + indent}px` }}
      >
        {entry.type === 'directory' ? (
          <span className="shrink-0 text-[var(--text-muted)]">
            {loading ? (
              <span className="inline-block size-3 animate-spin rounded-full border border-current border-t-transparent" />
            ) : expanded ? (
              <ChevronDown className="size-3" />
            ) : (
              <ChevronRight className="size-3" />
            )}
          </span>
        ) : (
          <span className="size-3 shrink-0" />
        )}
        <IconComp
          className={[
            isVirtualRoot ? 'size-3.5' : 'size-3',
            'shrink-0',
            entry.type === 'directory' ? dirIconColor : 'text-[var(--text-dim)]',
          ].join(' ')}
        />
        <span className="truncate">{entry.name}</span>
      </button>

      {expanded && children && children.length > 0 && (
        <div>
          <FileTree
            entries={children}
            selectedPath={selectedPath}
            onSelect={onSelect}
            depth={depth + 1}
          />
        </div>
      )}
      {expanded && children && children.length === 0 && (
        <div
          className="py-0.5 text-[10px] text-[var(--text-dim)] italic"
          style={{ paddingLeft: `${8 + indent + 20}px` }}
        >
          empty
        </div>
      )}
    </div>
  )
}

export function FileTree({ entries, selectedPath = null, onSelect, depth = 0 }: FileTreeProps) {
  return (
    <div className="select-none">
      {entries.map((entry) => (
        <TreeNode
          key={entry.path}
          entry={entry}
          selectedPath={selectedPath}
          onSelect={onSelect}
          depth={depth}
        />
      ))}
    </div>
  )
}
