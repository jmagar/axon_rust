'use client'

import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { type FileEntry, FileTree } from '@/components/workspace/file-tree'
import { apiFetch } from '@/lib/api-fetch'

export function WorkspaceSection() {
  const router = useRouter()
  const [entries, setEntries] = useState<FileEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [sortMode, setSortMode] = useState<'type' | 'name'>('type')

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    apiFetch('/api/workspace?action=list&path=')
      .then((res) => res.json())
      .then((data: { items?: FileEntry[] }) => {
        if (!cancelled) setEntries(data.items ?? [])
      })
      .catch(() => {
        if (!cancelled) setEntries([])
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [])

  const handleSelect = useCallback(
    (entry: FileEntry) => {
      setSelectedPath(entry.path)
      if (entry.type === 'file') {
        const url = `/editor?workspace=${encodeURIComponent(entry.path)}`
        router.push(url)
      }
    },
    [router],
  )

  const filteredEntries = useMemo(() => {
    const q = query.trim().toLowerCase()
    const visible = !q ? entries : entries.filter((entry) => entry.name.toLowerCase().includes(q))
    const sorted = [...visible]
    sorted.sort((a, b) => {
      if (sortMode === 'type' && a.type !== b.type) {
        return a.type === 'directory' ? -1 : 1
      }
      return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
    })
    return sorted
  }, [entries, query, sortMode])

  if (loading) {
    return (
      <div className="px-3 py-4 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
        Loading workspace...
      </div>
    )
  }

  if (entries.length === 0) {
    return (
      <div className="px-3 py-6 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
        Workspace is empty
      </div>
    )
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="space-y-1.5 border-b border-[var(--border-subtle)] px-2 py-2">
        <input
          type="text"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search files..."
          aria-label="Search workspace files"
          className="w-full rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.55)] px-2 py-1 text-[11px] text-[var(--text-secondary)] placeholder:text-[var(--text-dim)] focus:border-[var(--border-standard)] focus:outline-none"
        />
        <div className="flex items-center justify-between">
          <select
            value={sortMode}
            onChange={(event) => setSortMode(event.target.value as 'type' | 'name')}
            aria-label="Sort workspace files"
            className="rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.55)] px-2 py-1 text-[11px] text-[var(--text-secondary)] focus:border-[var(--border-standard)] focus:outline-none"
          >
            <option value="type">Folders first</option>
            <option value="name">Name A-Z</option>
          </select>
          <span className="text-[10px] text-[var(--text-dim)]">{filteredEntries.length} items</span>
        </div>
      </div>
      <FileTree entries={filteredEntries} selectedPath={selectedPath} onSelect={handleSelect} />
    </div>
  )
}
