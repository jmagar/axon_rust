import { z } from 'zod'

const ReplaceDocumentSchema = z.object({
  type: z.literal('replace_document'),
  markdown: z.string().min(1),
})

const AppendMarkdownSchema = z.object({
  type: z.literal('append_markdown'),
  markdown: z.string().min(1),
})

const InsertSectionSchema = z.object({
  type: z.literal('insert_section'),
  heading: z.string().min(1),
  markdown: z.string(),
  position: z.enum(['top', 'bottom']),
})

export const DocOperationSchema = z.discriminatedUnion('type', [
  ReplaceDocumentSchema,
  AppendMarkdownSchema,
  InsertSectionSchema,
])

export type DocOperation = z.infer<typeof DocOperationSchema>

export const PulsePermissionLevel = z.enum(['plan', 'training-wheels', 'full-access'])
export type PulsePermissionLevel = z.infer<typeof PulsePermissionLevel>

export const PulseChatRequestSchema = z.object({
  prompt: z.string().min(1).max(8000),
  documentMarkdown: z.string().max(100_000).default(''),
  selectedCollections: z.array(z.string()).default(['pulse']),
  conversationHistory: z
    .array(
      z.object({
        role: z.enum(['user', 'assistant']),
        content: z.string(),
      }),
    )
    .default([]),
  permissionLevel: PulsePermissionLevel.default('training-wheels'),
})

export type PulseChatRequest = z.infer<typeof PulseChatRequestSchema>

export interface PulseCitation {
  url: string
  title: string
  snippet: string
  collection: string
  score: number
}

export interface PulseChatResponse {
  text: string
  citations: PulseCitation[]
  operations: DocOperation[]
}

export interface PulseDocument {
  id: string
  title: string
  markdown: string
  createdAt: string
  updatedAt: string
  selectedCollections: string[]
  tags: string[]
}
