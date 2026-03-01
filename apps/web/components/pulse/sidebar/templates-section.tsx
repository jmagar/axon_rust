'use client'

import { FileText } from 'lucide-react'
import { useEffect, useState } from 'react'
import { ScrollArea } from '@/components/ui/scroll-area'

interface SkillEntry {
  name: string
  path: string
}

export function TemplatesSection() {
  const [skills, setSkills] = useState<SkillEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [selected, setSelected] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    fetch('/api/workspace?action=list&path=__claude/skills')
      .then((res) => res.json())
      .then((data: { items?: Array<{ name: string; path: string; type: string }> }) => {
        if (cancelled) return
        const files = (data.items ?? [])
          .filter((item) => item.type === 'file')
          .map((item) => ({ name: item.name, path: item.path }))
        setSkills(files)
      })
      .catch(() => {
        if (!cancelled) setSkills([])
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [])

  if (loading) {
    return (
      <div className="px-3 py-4 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
        Loading skills...
      </div>
    )
  }

  if (skills.length === 0) {
    return (
      <div className="px-3 py-6 text-center text-[length:var(--text-md)] text-[var(--text-dim)]">
        No skills found
      </div>
    )
  }

  return (
    <ScrollArea className="max-h-[30vh]">
      {skills.map((skill) => (
        <button
          key={skill.path}
          type="button"
          onClick={() => setSelected(selected === skill.path ? null : skill.path)}
          className={`flex w-full items-center gap-2 border-b border-[var(--border-subtle)] px-3 py-2 text-left transition-colors hover:bg-[var(--surface-float)] ${
            selected === skill.path
              ? 'bg-[rgba(135,175,255,0.08)] text-[var(--axon-primary)]'
              : 'text-[var(--text-secondary)]'
          }`}
        >
          <FileText className="size-3 flex-shrink-0 text-[var(--text-dim)]" />
          <span className="truncate text-[length:var(--text-md)]">{skill.name}</span>
        </button>
      ))}
    </ScrollArea>
  )
}
