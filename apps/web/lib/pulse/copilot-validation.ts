import { z } from 'zod'

export const CopilotRequestSchema = z.object({
  prompt: z.string().min(1),
  system: z.string().optional(),
  model: z.string().optional(),
})

export function validateCopilotRequest(body: unknown) {
  const result = CopilotRequestSchema.safeParse(body)
  return { valid: result.success, error: result.error?.message }
}
