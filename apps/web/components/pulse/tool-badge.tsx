'use client'

import { Bot, Command, File, Globe, Package, Plug2, Terminal, Zap } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

// ── Tool category taxonomy ─────────────────────────────────────────────────────

export type ToolCategory =
  | 'agent'
  | 'skill'
  | 'mcp'
  | 'file'
  | 'bash'
  | 'web'
  | 'plugin'
  | 'builtin'

type CategoryStyle = {
  border: string
  bg: string
  label: string
  categoryName: string
}

const CATEGORY_STYLES: Record<ToolCategory, CategoryStyle> = {
  agent: {
    border: 'border-[rgba(255,135,175,0.4)]',
    bg: 'bg-[rgba(20,5,15,0.7)]',
    label: 'text-[var(--axon-accent-pink)]',
    categoryName: 'Agent',
  },
  skill: {
    border: 'border-[rgba(167,139,250,0.4)]',
    bg: 'bg-[rgba(15,5,25,0.7)]',
    label: 'text-violet-300',
    categoryName: 'Skill',
  },
  mcp: {
    border: 'border-[rgba(34,211,238,0.4)]',
    bg: 'bg-[rgba(5,20,25,0.7)]',
    label: 'text-cyan-300',
    categoryName: 'MCP',
  },
  file: {
    border: 'border-[rgba(175,215,255,0.32)]',
    bg: 'bg-[rgba(10,15,30,0.7)]',
    label: 'text-[var(--axon-accent-blue)]',
    categoryName: 'File',
  },
  bash: {
    border: 'border-[rgba(245,158,11,0.4)]',
    bg: 'bg-[rgba(25,15,5,0.7)]',
    label: 'text-amber-300',
    categoryName: 'Bash',
  },
  web: {
    border: 'border-[rgba(45,212,191,0.4)]',
    bg: 'bg-[rgba(5,20,18,0.7)]',
    label: 'text-teal-300',
    categoryName: 'Web',
  },
  plugin: {
    border: 'border-[rgba(251,146,60,0.4)]',
    bg: 'bg-[rgba(25,12,5,0.7)]',
    label: 'text-orange-300',
    categoryName: 'Plugin',
  },
  builtin: {
    border: 'border-[rgba(148,163,184,0.32)]',
    bg: 'bg-[rgba(15,18,25,0.7)]',
    label: 'text-slate-300',
    categoryName: 'Tool',
  },
}

export function classifyTool(name: string): ToolCategory {
  if (name === 'Task') return 'agent'
  if (name === 'Skill') return 'skill'
  if (name.startsWith('mcp__')) return 'mcp'
  if (['Read', 'Write', 'Edit', 'Glob', 'Grep', 'LS'].includes(name)) return 'file'
  if (name === 'Bash') return 'bash'
  if (name === 'WebFetch' || name === 'WebSearch') return 'web'
  if (name.includes(':')) return 'plugin'
  return 'builtin'
}

function CategoryIcon({
  category,
  className = 'size-2.5',
}: {
  category: ToolCategory
  className?: string
}) {
  switch (category) {
    case 'agent':
      return <Bot className={className} />
    case 'skill':
      return <Zap className={className} />
    case 'mcp':
      return <Plug2 className={className} />
    case 'file':
      return <File className={className} />
    case 'bash':
      return <Terminal className={className} />
    case 'web':
      return <Globe className={className} />
    case 'plugin':
      return <Package className={className} />
    default:
      return <Command className={className} />
  }
}

// ── Public badge API ───────────────────────────────────────────────────────────

export type BadgeTool = { name: string; input: Record<string, unknown>; result?: string }

export function ToolCallBadge({ tool }: { tool: BadgeTool }) {
  const [open, setOpen] = useState(false)
  const [pinned, setPinned] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  const category = classifyTool(tool.name)
  const style = CATEGORY_STYLES[category]
  const isOpen = open || pinned

  useEffect(() => {
    if (!pinned) return
    function onOutsideClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setPinned(false)
        setOpen(false)
      }
    }
    document.addEventListener('mousedown', onOutsideClick)
    return () => document.removeEventListener('mousedown', onOutsideClick)
  }, [pinned])

  const displayName = tool.name.startsWith('mcp__')
    ? tool.name.split('__').slice(1).join(' › ')
    : tool.name

  const inputLines = Object.entries(tool.input)
    .slice(0, 4)
    .map(([k, v]) => ({
      key: k,
      val: (typeof v === 'string' ? v : (JSON.stringify(v) ?? '')).slice(0, 80),
    }))

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: tooltip wrapper, mouse events intentional
    <div
      ref={ref}
      className="relative inline-flex"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => {
        if (!pinned) setOpen(false)
      }}
    >
      <button
        type="button"
        onClick={() => setPinned((v) => !v)}
        className={`inline-flex size-5 items-center justify-center rounded border transition-colors duration-100 ${style.border} ${style.bg} ${style.label}`}
        aria-label={tool.name}
        title={tool.name}
      >
        <CategoryIcon category={category} />
      </button>

      {isOpen && (
        <div className="absolute bottom-full left-0 z-50 mb-1.5 w-52 rounded-lg border border-[rgba(255,255,255,0.1)] bg-[rgba(8,12,22,0.97)] shadow-[0_8px_24px_rgba(3,7,18,0.55)] backdrop-blur-sm">
          <div
            className={`flex items-center gap-1.5 border-b border-[rgba(255,255,255,0.07)] px-2 py-1.5`}
          >
            <span
              className={`inline-flex size-3.5 shrink-0 items-center justify-center rounded border ${style.border} ${style.bg}`}
            >
              <CategoryIcon category={category} className="size-2" />
            </span>
            <span
              className={`min-w-0 flex-1 truncate text-[length:var(--text-xs)] font-semibold ${style.label}`}
            >
              {displayName}
            </span>
            <span className="shrink-0 rounded border border-[rgba(255,255,255,0.1)] px-1 py-0.5 text-[length:var(--text-2xs)] text-[var(--axon-text-dim)]">
              {style.categoryName}
            </span>
          </div>

          {inputLines.length > 0 && (
            <div className="space-y-0.5 px-2 py-1.5">
              {inputLines.map(({ key, val }) => (
                <div
                  key={key}
                  className="grid grid-cols-[auto_1fr] gap-1.5 text-[length:var(--text-2xs)]"
                >
                  <span className="shrink-0 text-[var(--axon-text-dim)]">{key}</span>
                  <span className="truncate text-[var(--axon-text-secondary)]">{val}</span>
                </div>
              ))}
            </div>
          )}

          {tool.result && (
            <div className="border-t border-[rgba(255,255,255,0.07)] px-2 py-1.5">
              <p className="mb-0.5 text-[length:var(--text-2xs)] text-[var(--axon-text-dim)]">
                Result
              </p>
              <p className="line-clamp-3 text-[length:var(--text-2xs)] text-[var(--axon-text-secondary)]">
                {tool.result}
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
