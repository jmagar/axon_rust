/**
 * Pure async API helpers for the Pulse chat system.
 * No React imports — safe to use from hooks and server utilities.
 */

import { parsePulseChatStreamChunk } from '@/lib/pulse/chat-stream'
import type {
  PulseChatResponse,
  PulseModel,
  PulsePermissionLevel,
  PulseSourceResponse,
  PulseToolUse,
} from '@/lib/pulse/types'

export interface ChatStreamEvent {
  type: 'status' | 'assistant_delta' | 'tool_use' | 'thinking_content'
  phase?: 'started' | 'thinking' | 'finalizing'
  delta?: string
  tool?: PulseToolUse
  content?: string
}

export interface RunChatPromptOptions {
  prompt: string
  conversationHistory: Array<{ role: 'user' | 'assistant'; content: string }>
  signal: AbortSignal
  onEvent?: (event: ChatStreamEvent) => void
  chatSessionId: string | null
  documentMarkdown: string
  activeThreadSources: string[]
  scrapedContext: { url: string; markdown: string } | null
  permissionLevel: PulsePermissionLevel
  model: PulseModel
  effort?: string
  maxTurns?: number
  maxBudgetUsd?: number
  appendSystemPrompt?: string
  disableSlashCommands?: boolean
  noSessionPersistence?: boolean
  fallbackModel?: string
  allowedTools?: string
  disallowedTools?: string
  addDir?: string
  betas?: string
  toolsRestrict?: string
}

async function readNdjsonStream(
  response: Response,
  onEvent?: (event: ChatStreamEvent) => void,
): Promise<PulseChatResponse> {
  const reader = response.body!.getReader()
  const decoder = new TextDecoder()
  let remainder = ''
  let doneResponse: PulseChatResponse | null = null
  const seenEventIds = new Set<string>()

  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    const chunk = decoder.decode(value, { stream: true })
    const parsed = parsePulseChatStreamChunk(chunk, remainder)
    remainder = parsed.remainder

    for (const event of parsed.events) {
      if (seenEventIds.has(event.event_id)) continue
      seenEventIds.add(event.event_id)
      if (event.type === 'status') {
        onEvent?.({ type: 'status', phase: event.phase })
        continue
      }
      if (event.type === 'assistant_delta') {
        onEvent?.({ type: 'assistant_delta', delta: event.delta })
        continue
      }
      if (event.type === 'thinking_content') {
        onEvent?.({ type: 'thinking_content', content: event.content })
        continue
      }
      if (event.type === 'tool_use') {
        onEvent?.({ type: 'tool_use', tool: event.tool })
        continue
      }
      if (event.type === 'error') {
        throw new Error(event.error || 'Pulse stream error')
      }
      if (event.type === 'done') {
        doneResponse = event.response
      }
    }
  }

  const finalTail = remainder.trim()
  if (finalTail) {
    try {
      const parsedEvent = JSON.parse(finalTail) as { type?: string; event_id?: string }
      if (typeof parsedEvent.event_id === 'string' && seenEventIds.has(parsedEvent.event_id)) {
        if (!doneResponse) throw new Error('Pulse stream ended without a final response')
        return doneResponse
      }
      if (parsedEvent.type === 'done') {
        doneResponse = (parsedEvent as { response: PulseChatResponse }).response
      } else if (parsedEvent.type === 'error') {
        const message = (parsedEvent as { error?: string }).error ?? 'Pulse stream error'
        throw new Error(message)
      }
    } catch {
      // Ignore trailing malformed NDJSON fragments.
    }
  }

  if (!doneResponse) {
    throw new Error('Pulse stream ended without a final response')
  }
  return doneResponse
}

export async function runChatPrompt(opts: RunChatPromptOptions): Promise<PulseChatResponse> {
  const {
    prompt,
    conversationHistory,
    signal,
    onEvent,
    chatSessionId,
    documentMarkdown,
    activeThreadSources,
    scrapedContext,
    permissionLevel,
    model,
    effort,
    maxTurns,
    maxBudgetUsd,
    appendSystemPrompt,
    disableSlashCommands,
    noSessionPersistence,
    fallbackModel,
    allowedTools,
    disallowedTools,
    addDir,
    betas,
    toolsRestrict,
  } = opts

  const response = await fetch('/api/pulse/chat', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    signal,
    body: JSON.stringify({
      prompt,
      sessionId: chatSessionId ?? undefined,
      documentMarkdown,
      selectedCollections: ['cortex'],
      threadSources: activeThreadSources,
      scrapedContext: scrapedContext ?? undefined,
      conversationHistory,
      permissionLevel,
      model,
      effort,
      maxTurns,
      maxBudgetUsd,
      appendSystemPrompt,
      disableSlashCommands,
      noSessionPersistence,
      fallbackModel,
      allowedTools,
      disallowedTools,
      addDir,
      betas,
      toolsRestrict,
    }),
  })

  if (!response.ok) {
    const errorBody = await response.text()
    let detail = ''
    if (errorBody) {
      try {
        const parsed = JSON.parse(errorBody) as { error?: unknown; message?: unknown }
        detail =
          typeof parsed.error === 'string'
            ? parsed.error
            : typeof parsed.message === 'string'
              ? parsed.message
              : errorBody
      } catch {
        detail = errorBody
      }
    }
    const suffix = detail ? `: ${detail}` : ''
    throw new Error(`Pulse chat failed (${response.status})${suffix}`)
  }

  const contentType = response.headers.get('content-type')?.toLowerCase() ?? ''
  const isNdjson =
    contentType.includes('application/x-ndjson') || contentType.includes('application/ndjson')

  if (isNdjson && response.body) {
    return readNdjsonStream(response, onEvent)
  }

  return (await response.json()) as PulseChatResponse
}

export async function runSourcePrompt(
  urls: string[],
  signal: AbortSignal,
): Promise<PulseSourceResponse> {
  const response = await fetch('/api/pulse/source', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    signal,
    body: JSON.stringify({ urls }),
  })
  if (!response.ok) {
    const body = await response.text()
    throw new Error(body || `Source ingest failed (${response.status})`)
  }
  return (await response.json()) as PulseSourceResponse
}
