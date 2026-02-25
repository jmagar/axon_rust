import { describe, expect, it } from 'vitest'
import {
  type DocOperation,
  DocOperationSchema,
  PulseChatRequestSchema,
  PulsePermissionLevel,
} from '@/lib/pulse/types'

describe('pulse types', () => {
  it('validates a replace_document op', () => {
    const op: DocOperation = { type: 'replace_document', markdown: '# Hello' }
    expect(DocOperationSchema.parse(op)).toEqual(op)
  })

  it('validates an append_markdown op', () => {
    const op: DocOperation = { type: 'append_markdown', markdown: 'Some text' }
    expect(DocOperationSchema.parse(op)).toEqual(op)
  })

  it('validates an insert_section op', () => {
    const op: DocOperation = {
      type: 'insert_section',
      heading: 'New Section',
      markdown: 'Content here',
      position: 'bottom',
    }
    expect(DocOperationSchema.parse(op)).toEqual(op)
  })

  it('rejects unknown op types', () => {
    expect(() => DocOperationSchema.parse({ type: 'delete_everything', markdown: '' })).toThrow()
  })

  it('validates a chat request', () => {
    const req = {
      prompt: 'Add a summary section',
      documentMarkdown: '# Doc\n\nContent here',
      selectedCollections: ['pulse', 'cortex'],
    }
    expect(PulseChatRequestSchema.parse(req)).toBeTruthy()
  })

  it('rejects empty prompt', () => {
    expect(() => PulseChatRequestSchema.parse({ prompt: '' })).toThrow()
  })

  it('rejects prompt over 8000 chars', () => {
    expect(() => PulseChatRequestSchema.parse({ prompt: 'X'.repeat(8001) })).toThrow()
  })

  it('permission levels are correct', () => {
    expect(PulsePermissionLevel.options).toEqual(['plan', 'training-wheels', 'full-access'])
  })
})
