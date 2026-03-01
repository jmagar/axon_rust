'use client'

import { ChevronRight } from 'lucide-react'
import type { FileEntry } from './file-tree'

export function WorkspaceBreadcrumb({ entry }: { entry: FileEntry | null }) {
  if (!entry) {
    return <span className="text-[var(--text-dim)] text-xs font-mono">Files</span>
  }
  if (entry.path.startsWith('__')) {
    return <span className="text-[var(--text-secondary)] text-xs font-mono">{entry.name}</span>
  }
  const parts = entry.path.split('/').filter(Boolean)
  return (
    <div className="flex items-center gap-1 font-mono text-xs overflow-x-auto">
      <span className="text-[var(--text-muted)] shrink-0">Workspace</span>
      {parts.map((part, i) => {
        const partPath = parts.slice(0, i + 1).join('/')
        const isLast = i === parts.length - 1
        return (
          <span key={partPath} className="flex items-center gap-1 shrink-0">
            <ChevronRight className="size-3 text-[var(--text-dim)]" />
            <span className={isLast ? 'text-[var(--axon-secondary)]' : 'text-[var(--text-muted)]'}>
              {part}
            </span>
          </span>
        )
      })}
    </div>
  )
}
