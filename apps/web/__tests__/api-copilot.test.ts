import { describe, expect, it } from 'vitest'
import { validateCopilotRequest } from '@/lib/pulse/copilot-validation'

describe('copilot request validation', () => {
  it('rejects empty prompt', () => {
    expect(validateCopilotRequest({ prompt: '' }).valid).toBe(false)
  })

  it('accepts valid prompt', () => {
    expect(validateCopilotRequest({ prompt: 'Continue: The quick brown' }).valid).toBe(true)
  })

  it('accepts prompt with optional system message', () => {
    const result = validateCopilotRequest({
      prompt: 'Continue this text',
      system: 'You are a writing assistant.',
    })
    expect(result.valid).toBe(true)
  })
})
