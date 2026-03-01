import { z } from 'zod'

export const CopilotRequestSchema = z.object({
  prompt: z.string().min(1).max(8000),
  system: z.string().max(4000).optional(),
  model: z.string().max(100).optional(),
})

export interface CopilotValidationResult {
  valid: boolean
  error?: string
}

export function validateCopilotRequest(body: unknown): CopilotValidationResult {
  const result = CopilotRequestSchema.safeParse(body)
  if (result.success) {
    return { valid: true }
  }
  const firstIssue = result.error.issues[0]
  return {
    valid: false,
    error: firstIssue
      ? `${firstIssue.path.join('.') || 'request'}: ${firstIssue.message}`
      : 'Invalid request',
  }
}
