'use client'

import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useState } from 'react'
import { type FileEntry, FileTree } from '@/components/workspace/file-tree'

export function WorkspaceSection() {
  const router = useRouter()
  const [entries, setEntries] = useState<FileEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [selectedPath, setSelectedPath] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    fetch('/api/workspace?action=list&path=')
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
      <FileTree entries={entries} selectedPath={selectedPath} onSelect={handleSelect} />
    </div>
  )
}
