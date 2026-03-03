'use client'

import { useEffect } from 'react'
import { createPortal } from 'react-dom'

import type { ValidationResult } from '@/lib/pulse/doc-ops'
import type { DocOperation } from '@/lib/pulse/types'

interface PulseOpConfirmationProps {
  operations: DocOperation[]
  validation: ValidationResult
  onConfirm: () => void
  onReject: () => void
}

const REASON_LABELS: Record<string, string> = {
  too_many_ops: 'Multiple operations in one response',
  large_insert: 'Large text insertion (>1200 characters)',
  large_replace: 'Replaces more than 40% of the document',
  removes_heading: 'Removes one or more section headings',
}

export function PulseOpConfirmation({
  operations,
  validation,
  onConfirm,
  onReject,
}: PulseOpConfirmationProps) {
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || (e.target as HTMLElement)?.isContentEditable) {
        return
      }
      if (e.key === 'Escape') onReject()
      if (e.key === 'Enter') onConfirm()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onConfirm, onReject])

  return createPortal(
    <div
      className="fixed inset-0 z-[9999] flex items-center justify-center bg-black/40"
      onClick={(e) => {
        if (e.target === e.currentTarget) onReject()
      }}
      onKeyDown={undefined}
      role="dialog"
      aria-modal="true"
      aria-label="Confirm document changes"
    >
      <div className="mx-4 w-full max-w-md rounded-xl border border-[rgba(175,215,255,0.3)] bg-[var(--bg-primary,#0a1223)] p-6 shadow-2xl shadow-black/40">
        <h4 className="mb-3 text-sm font-bold uppercase tracking-wider text-[var(--axon-primary-strong)]">
          Confirm Document Changes
        </h4>
        <p className="mb-4 text-sm text-[var(--text-muted)]">
          The assistant wants to apply {operations.length} operation(s) that triggered safety
          checks:
        </p>
        <ul className="mb-4 space-y-1.5">
          {validation.reasons.map((reason) => (
            <li key={reason} className="text-sm text-[var(--text-secondary)]">
              &bull; {REASON_LABELS[reason] ?? reason}
            </li>
          ))}
        </ul>
        <div className="flex gap-3">
          <button
            type="button"
            onClick={onConfirm}
            className="rounded-md bg-[rgba(175,215,255,0.2)] px-4 py-2 text-sm font-semibold text-[var(--axon-primary-strong)] transition-colors hover:bg-[rgba(175,215,255,0.35)]"
          >
            Apply Changes
          </button>
          <button
            type="button"
            onClick={onReject}
            className="rounded-md bg-[rgba(255,135,175,0.1)] px-4 py-2 text-sm font-semibold text-[var(--text-muted)] transition-colors hover:text-[var(--axon-secondary)]"
          >
            Reject
          </button>
        </div>
        <p className="mt-3 text-[11px] text-[var(--text-dim)]">
          Press Enter to apply &middot; Esc to reject
        </p>
      </div>
    </div>,
    document.body,
  )
}
