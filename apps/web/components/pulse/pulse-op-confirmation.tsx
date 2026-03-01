'use client'

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
  return (
    <div className="rounded-lg border border-[rgba(175,215,255,0.3)] bg-[rgba(175,215,255,0.05)] p-4">
      <h4 className="mb-2 text-xs font-bold uppercase tracking-wider text-[var(--axon-primary-strong)]">
        Confirm Document Changes
      </h4>
      <p className="mb-3 text-xs text-[var(--text-muted)]">
        The assistant wants to apply {operations.length} operation(s) that triggered safety checks:
      </p>
      <ul className="mb-3 space-y-1">
        {validation.reasons.map((reason) => (
          <li key={reason} className="text-xs text-[var(--text-secondary)]">
            {REASON_LABELS[reason] ?? reason}
          </li>
        ))}
      </ul>
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onConfirm}
          className="rounded-md bg-[rgba(175,215,255,0.2)] px-3 py-1.5 text-xs font-semibold text-[var(--axon-primary-strong)] transition-colors hover:bg-[rgba(175,215,255,0.3)]"
        >
          Apply Changes
        </button>
        <button
          type="button"
          onClick={onReject}
          className="rounded-md bg-[rgba(255,135,175,0.1)] px-3 py-1.5 text-xs font-semibold text-[var(--text-muted)] transition-colors hover:text-[var(--axon-secondary)]"
        >
          Reject
        </button>
      </div>
    </div>
  )
}
