import { z } from 'zod'

const ReplaceDocumentSchema = z.object({
  type: z.literal('replace_document'),
  markdown: z.string().min(1).max(100_000),
})

const AppendMarkdownSchema = z.object({
  type: z.literal('append_markdown'),
  markdown: z.string().min(1).max(100_000),
})

const InsertSectionSchema = z.object({
  type: z.literal('insert_section'),
  heading: z.string().min(1),
  markdown: z.string().max(100_000),
  position: z.enum(['top', 'bottom']),
})

export const DocOperationSchema = z.discriminatedUnion('type', [
  ReplaceDocumentSchema,
  AppendMarkdownSchema,
  InsertSectionSchema,
])

export type DocOperation = z.infer<typeof DocOperationSchema>

export const PulsePermissionLevel = z.enum(['plan', 'accept-edits', 'bypass-permissions'])
export type PulsePermissionLevel = z.infer<typeof PulsePermissionLevel>

export const PulseModel = z.enum(['sonnet', 'opus', 'haiku'])
export type PulseModel = z.infer<typeof PulseModel>

export const PulseChatRequestSchema = z.object({
  prompt: z.string().min(1).max(8000),
  sessionId: z.string().min(1).max(256).optional(),
  documentMarkdown: z.string().max(100_000).default(''),
  selectedCollections: z.array(z.string().min(1).max(100)).max(10).default(['cortex']),
  threadSources: z.array(z.string().url()).max(25).default([]),
  /** Markdown from the most recent scrape — injected directly into the system prompt. */
  scrapedContext: z.object({ url: z.string(), markdown: z.string().max(40_000) }).optional(),
  conversationHistory: z
    .array(
      z.object({
        role: z.enum(['user', 'assistant']),
        content: z.string().max(8_000),
      }),
    )
    .max(50)
    .default([]),
  permissionLevel: PulsePermissionLevel.default('accept-edits'),
  model: PulseModel.default('sonnet'),
})

export type PulseChatRequest = z.infer<typeof PulseChatRequestSchema>

export interface PulseCitation {
  url: string
  title: string
  snippet: string
  collection: string
  score: number
}

export interface PulseToolUse {
  name: string
  input: Record<string, unknown>
}

export type PulseMessageBlock =
  | { type: 'text'; content: string }
  | { type: 'tool_use'; name: string; input: Record<string, unknown>; result?: string }

export interface PulseChatResponse {
  text: string
  sessionId?: string
  citations: PulseCitation[]
  operations: DocOperation[]
  toolUses: PulseToolUse[]
  blocks: PulseMessageBlock[]
  metadata?: {
    model: PulseModel
    elapsedMs: number
    systemPromptChars: number
    promptChars: number
    documentChars: number
    conversationChars: number
    citationChars: number
    threadSourceChars: number
    contextCharsTotal: number
    contextBudgetChars: number
  }
}

export const PulseSourceRequestSchema = z.object({
  urls: z.array(z.string().url()).min(1).max(10),
})

export type PulseSourceRequest = z.infer<typeof PulseSourceRequestSchema>

export interface PulseSourceResponse {
  indexed: string[]
  command: string
  output: string
  /** Scraped markdown keyed by URL — available when a single URL is indexed. */
  markdownBySrc?: Record<string, string>
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
