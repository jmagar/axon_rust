'use client'

import { FilePen, FilePlus, FileText } from 'lucide-react'
import type React from 'react'
import { useEffect, useRef, useState } from 'react'

// ── Doc-op badge (post-processed document operations) ─────────────────────────

const DOC_OP_META: Record<string, { label: string; Icon: React.FC<{ className?: string }> }> = {
  replace_document: { label: 'Replace doc', Icon: FilePen },
  append_markdown: { label: 'Append', Icon: FilePlus },
  insert_section: { label: 'Insert section', Icon: FileText },
}

export function DocOpBadge({ type, heading }: { type: string; heading?: string }) {
  const [open, setOpen] = useState(false)
  const [pinned, setPinned] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  const meta = DOC_OP_META[type] ?? { label: type, Icon: FileText }
  const { label, Icon } = meta
  const displayLabel = type === 'insert_section' && heading ? `Insert · ${heading}` : label
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
        className="inline-flex size-5 items-center justify-center rounded border border-[rgba(52,211,153,0.4)] bg-[rgba(5,20,10,0.7)] text-emerald-300 transition-colors duration-100"
        aria-label={displayLabel}
        title={displayLabel}
      >
        <Icon className="size-2.5" />
      </button>

      {isOpen && (
        <div className="absolute bottom-full left-0 z-50 mb-1.5 w-44 rounded-lg border border-[rgba(255,255,255,0.1)] bg-[rgba(8,12,22,0.97)] shadow-[0_8px_24px_rgba(3,7,18,0.55)] backdrop-blur-sm">
          <div className="flex items-center gap-1.5 px-2 py-1.5">
            <span className="inline-flex size-3.5 shrink-0 items-center justify-center rounded border border-[rgba(52,211,153,0.4)] bg-[rgba(5,20,10,0.7)]">
              <Icon className="size-2 text-emerald-300" />
            </span>
            <span className="min-w-0 flex-1 truncate text-[length:var(--text-xs)] font-semibold text-emerald-300">
              {displayLabel}
            </span>
            <span className="shrink-0 rounded border border-[rgba(255,255,255,0.1)] px-1 py-0.5 text-[length:var(--text-2xs)] text-[var(--text-dim)]">
              Doc op
            </span>
          </div>
        </div>
      )}
    </div>
  )
}
