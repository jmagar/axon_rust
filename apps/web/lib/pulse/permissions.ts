import { isHighRiskOperationSet } from './doc-ops'
import type { DocOperation, PulsePermissionLevel } from './types'

interface PermissionContext {
  isCurrentDoc: boolean
  currentDocMarkdown?: string
}

interface PermissionResult {
  allowed: boolean
  requiresConfirmation: boolean
  reason?: string
}

export function checkPermission(
  level: PulsePermissionLevel,
  operations: DocOperation[],
  context: PermissionContext,
): PermissionResult {
  if (level === 'bypass-permissions') {
    return { allowed: true, requiresConfirmation: false }
  }

  if (level === 'plan') {
    return { allowed: false, requiresConfirmation: false, reason: 'plan_mode_no_edits' }
  }

  if (level === 'accept-edits') {
    if (!context.isCurrentDoc) {
      return {
        allowed: false,
        requiresConfirmation: false,
        reason: 'accept_edits_current_doc_only',
      }
    }
    const highRisk = isHighRiskOperationSet(operations, context.currentDocMarkdown ?? '')
    return { allowed: true, requiresConfirmation: highRisk }
  }

  return { allowed: true, requiresConfirmation: false }
}
