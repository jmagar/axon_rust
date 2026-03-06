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

export const AcpConfigSelectValue = z.object({
  value: z.string(),
  name: z.string(),
  description: z.string().optional(),
})
export type AcpConfigSelectValue = z.infer<typeof AcpConfigSelectValue>

export const AcpConfigOption = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().optional(),
  category: z.string().optional(),
  currentValue: z.string(),
  options: z.array(AcpConfigSelectValue),
})
export type AcpConfigOption = z.infer<typeof AcpConfigOption>

export const PulseModel = z.string().default('sonnet')
export type PulseModel = z.infer<typeof PulseModel>
export const PulseAgent = z.enum(['claude', 'codex'])
export type PulseAgent = z.infer<typeof PulseAgent>

export const PulseChatRequestSchema = z.object({
  prompt: z.string().min(1).max(8000),
  sessionId: z
    .string()
    .regex(/^[0-9a-f-]{8,64}$/i)
    .optional(),
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
  agent: PulseAgent.default('claude'),
  model: PulseModel.default('sonnet'),
  effort: z.enum(['low', 'medium', 'high']).default('medium'),
  maxTurns: z.number().int().min(0).max(100).default(0),
  maxBudgetUsd: z.number().min(0).max(1000).default(0),
  appendSystemPrompt: z.string().max(4000).default(''),
  // Additional CLI flags from Claude Code docs
  /** --disable-slash-commands: disable all skills and slash commands */
  disableSlashCommands: z.boolean().default(false),
  /** --no-session-persistence: sessions are not saved to disk */
  noSessionPersistence: z.boolean().default(false),
  /** --fallback-model: fallback model alias when primary is overloaded ('' = disabled) */
  fallbackModel: z.string().max(128).default(''),
  /** --allowedTools: comma-separated tools that execute without permission prompts */
  allowedTools: z.string().max(2000).default(''),
  /** --disallowedTools: comma-separated tools removed from model context */
  disallowedTools: z.string().max(2000).default(''),
  /** --add-dir: comma-separated directories Claude can access beyond the working dir */
  addDir: z.string().optional(),
  /** --betas: comma-separated beta headers (e.g. interleaved-thinking) */
  betas: z
    .string()
    .regex(/^[a-zA-Z0-9,\-.:]*$/, 'betas contains invalid characters')
    .optional(),
  /** --tools: restrict which built-in tools are available */
  toolsRestrict: z
    .string()
    .regex(/^[a-zA-Z0-9,\-.:]*$/, 'toolsRestrict contains invalid characters')
    .optional(),
  /** Stream replay: resume from this event ID (supports both camelCase and snake_case). */
  lastEventId: z.string().max(128).optional(),
  last_event_id: z.string().max(128).optional(),
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
  | { type: 'thinking'; content: string }

export interface PulseChatResponse {
  text: string
  sessionId?: string
  citations: PulseCitation[]
  operations: DocOperation[]
  toolUses: PulseToolUse[]
  blocks: PulseMessageBlock[]
  metadata?: {
    model: PulseModel
    agent?: PulseAgent
    elapsedMs: number
    contextCharsTotal: number
    contextBudgetChars: number
    first_delta_ms?: number | null
    time_to_done_ms?: number
    delta_count?: number
    aborted?: boolean
    fallback_source?: 'conversation_memory'
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
