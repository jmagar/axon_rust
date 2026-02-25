import type { DocOperation } from './types'

const REPLACE_CHAR_THRESHOLD = 0.4
const MAX_INSERT_CHARS = 1200
const MAX_OPS_PER_RESPONSE = 3

export interface ValidationResult {
  valid: boolean
  reasons: string[]
}

function extractHeadings(markdown: string): string[] {
  return markdown
    .split('\n')
    .filter((line) => /^#{1,6}\s+/.test(line))
    .map((line) => line.trim().toLowerCase())
}

function estimateChangedRatio(original: string, replacement: string): number {
  if (!original.length && !replacement.length) return 0
  if (!original.length) return 1
  let changed = 0
  const minLen = Math.min(original.length, replacement.length)
  for (let i = 0; i < minLen; i += 1) {
    if (original[i] !== replacement[i]) changed += 1
  }
  changed += Math.abs(original.length - replacement.length)
  return changed / Math.max(original.length, replacement.length, 1)
}

export function validateDocOperations(
  operations: DocOperation[],
  originalMarkdown: string,
): ValidationResult {
  const reasons = new Set<string>()

  if (operations.length > MAX_OPS_PER_RESPONSE) {
    reasons.add('too_many_ops')
  }

  for (const op of operations) {
    if (
      (op.type === 'append_markdown' || op.type === 'insert_section') &&
      op.markdown.length > MAX_INSERT_CHARS
    ) {
      reasons.add('large_insert')
    }

    if (op.type === 'replace_document') {
      if (estimateChangedRatio(originalMarkdown, op.markdown) > REPLACE_CHAR_THRESHOLD) {
        reasons.add('large_replace')
      }

      const beforeHeadings = extractHeadings(originalMarkdown)
      const afterHeadings = extractHeadings(op.markdown)
      const removed = beforeHeadings.some((heading) => !afterHeadings.includes(heading))
      if (removed) {
        reasons.add('removes_heading')
      }
    }
  }

  return { valid: reasons.size === 0, reasons: Array.from(reasons) }
}

export function isHighRiskOperationSet(
  operations: DocOperation[],
  originalMarkdown: string,
): boolean {
  return !validateDocOperations(operations, originalMarkdown).valid
}
