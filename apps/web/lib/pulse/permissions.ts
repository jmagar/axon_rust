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
  if (level === 'full-access') {
    return { allowed: true, requiresConfirmation: false }
  }

  if (level === 'plan' && !context.isCurrentDoc) {
    return { allowed: false, requiresConfirmation: false, reason: 'plan_mode_current_doc_only' }
  }

  if (level === 'training-wheels') {
    const highRisk = isHighRiskOperationSet(operations, context.currentDocMarkdown ?? '')
    return { allowed: true, requiresConfirmation: highRisk }
  }

  return { allowed: true, requiresConfirmation: false }
}
