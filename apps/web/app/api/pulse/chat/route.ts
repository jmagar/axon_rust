import { runAxonCommandWsStream } from '@/lib/axon-ws-exec'
import {
  createPulseChatStreamEvent,
  encodePulseChatStreamEvent,
  type PulseChatStreamEvent,
} from '@/lib/pulse/chat-stream'
import { fallbackAssistantText, parseClaudeAssistantPayload } from '@/lib/pulse/claude-response'
import { resolveConversationMemoryAnswer } from '@/lib/pulse/conversation-memory'
import { checkPermission } from '@/lib/pulse/permissions'
import { buildPulseSystemPrompt, retrieveFromCollections } from '@/lib/pulse/rag'
import { ensureRepoRootEnvLoaded } from '@/lib/pulse/server-env'
import {
  DocOperationSchema,
  type PulseChatRequest,
  PulseChatRequestSchema,
  type PulseChatResponse,
  type PulseCitation,
} from '@/lib/pulse/types'
import { apiError, makeErrorId } from '@/lib/server/api-error'
import {
  CLAUDE_TIMEOUT_MS,
  computeContextCharsTotal,
  GLOBAL_CLAUDE_MD_CHARS,
  HEARTBEAT_INTERVAL_MS,
  MODEL_CONTEXT_BUDGET_CHARS,
} from './claude-stream-types'
import {
  computeReplayKey,
  pruneReplayCache,
  REPLAY_BUFFER_LIMIT,
  replayCache,
  upsertReplayEntry,
} from './replay-cache'
import { createStreamParserState, extractToolResultText } from './stream-parser'

const DEFAULT_PULSE_CHAT_TIMEOUT_MS = CLAUDE_TIMEOUT_MS

function resolvePulseChatTimeoutMs(): number {
  const raw = process.env.AXON_PULSE_CHAT_TIMEOUT_MS
  if (!raw) return DEFAULT_PULSE_CHAT_TIMEOUT_MS
  const parsed = Number(raw)
  if (!Number.isFinite(parsed) || parsed <= 0) return DEFAULT_PULSE_CHAT_TIMEOUT_MS
  return Math.floor(parsed)
}

// ── Request preparation (extracted from POST for readability) ────────────────

function buildPromptText(userPrompt: string): string {
  return userPrompt
}

function parseOperations(result: string): {
  text: string
  operations: PulseChatResponse['operations']
} {
  const parsedPayload = parseClaudeAssistantPayload(result)
  if (!parsedPayload) {
    return { text: fallbackAssistantText(result), operations: [] }
  }

  const operations: PulseChatResponse['operations'] = []
  for (const op of parsedPayload.operations) {
    const parsedOp = DocOperationSchema.safeParse(op)
    if (parsedOp.success) {
      operations.push(parsedOp.data)
    }
  }
  return { text: parsedPayload.text, operations }
}

function buildDoneResponse(
  req: PulseChatRequest,
  citations: PulseCitation[],
  parserState: ReturnType<typeof createStreamParserState>,
  startedAt: number,
  contextCharsTotal: number,
  aborted: boolean,
): Parameters<typeof createPulseChatStreamEvent>[0] {
  const elapsed = Date.now() - startedAt
  const telemetry = {
    elapsedMs: elapsed,
    contextCharsTotal,
    contextBudgetChars: MODEL_CONTEXT_BUDGET_CHARS,
    first_delta_ms: parserState.firstDeltaMs,
    time_to_done_ms: elapsed,
    delta_count: parserState.deltaCount,
    aborted,
  }

  if (aborted) {
    return {
      type: 'done',
      response: {
        text: fallbackAssistantText(parserState.result),
        sessionId: parserState.sessionId ?? undefined,
        citations,
        operations: [],
        toolUses: parserState.toolUses,
        blocks: parserState.blocks,
        metadata: { agent: req.agent, model: req.model, ...telemetry },
      },
    }
  }

  let { text, operations } = parseOperations(parserState.result)

  const permission = checkPermission(req.permissionLevel, operations, {
    isCurrentDoc: true,
    currentDocMarkdown: req.documentMarkdown,
  })
  if (!permission.allowed) {
    operations = []
    text = text || 'Operation blocked by permission policy.'
  }

  return {
    type: 'done',
    response: {
      text,
      sessionId: parserState.sessionId ?? undefined,
      citations,
      operations,
      toolUses: parserState.toolUses,
      blocks: parserState.blocks,
      metadata: { agent: req.agent, model: req.model, ...telemetry },
    },
  }
}

function recordAssistantDelta(
  parserState: ReturnType<typeof createStreamParserState>,
  delta: string,
  startedAt: number,
): void {
  parserState.blocks.push({ type: 'text', content: delta })
  parserState.deltaCount += 1
  if (parserState.firstDeltaMs === null) {
    parserState.firstDeltaMs = Date.now() - startedAt
  }
}

function recordThinking(
  parserState: ReturnType<typeof createStreamParserState>,
  content: string,
): void {
  const lastBlock = parserState.blocks[parserState.blocks.length - 1]
  if (lastBlock?.type === 'thinking') {
    lastBlock.content = content
    return
  }
  parserState.blocks.push({ type: 'thinking', content })
}

function upsertToolUse(
  parserState: ReturnType<typeof createStreamParserState>,
  id: string | undefined,
  name: string,
  input: Record<string, unknown>,
): void {
  if (id) {
    const existingIdx = parserState.toolUseIdToIdx.get(id)
    if (existingIdx !== undefined) {
      const existingBlock = parserState.blocks[existingIdx]
      if (existingBlock?.type === 'tool_use') {
        existingBlock.input = input
      }
      const toolIdx = parserState.blocks
        .slice(0, existingIdx)
        .filter((b) => b.type === 'tool_use').length
      const existingTool = parserState.toolUses[toolIdx]
      if (existingTool) {
        existingTool.input = input
      }
      return
    }
  }

  const idx = parserState.blocks.length
  parserState.blocks.push({ type: 'tool_use', name, input })
  if (id) parserState.toolUseIdToIdx.set(id, idx)
  parserState.toolUses.push({ name, input })
}

function patchToolResult(
  parserState: ReturnType<typeof createStreamParserState>,
  toolUseId: string,
  resultText: string,
): void {
  const idx = parserState.toolUseIdToIdx.get(toolUseId)
  if (idx === undefined) return
  const block = parserState.blocks[idx]
  if (block?.type !== 'tool_use') return
  block.result = resultText.slice(0, 600)
}

// ── POST handler ─────────────────────────────────────────────────────────────

export async function POST(request: Request) {
  ensureRepoRootEnvLoaded()
  const startedAt = Date.now()

  let body: unknown
  try {
    body = await request.json()
  } catch {
    return apiError(400, 'Request body must be valid JSON')
  }

  try {
    const parsed = PulseChatRequestSchema.safeParse(body)
    if (!parsed.success) {
      return apiError(400, parsed.error.issues[0]?.message ?? 'Invalid request payload')
    }

    const req = parsed.data
    // last_event_id / lastEventId — now validated through Zod instead of raw body cast
    const lastEventId = req.last_event_id ?? req.lastEventId

    const replayKey = computeReplayKey({
      prompt: req.prompt,
      documentMarkdown: req.documentMarkdown,
      selectedCollections: req.selectedCollections,
      threadSources: req.threadSources,
      scrapedContext: req.scrapedContext,
      conversationHistory: req.conversationHistory,
      permissionLevel: req.permissionLevel,
      agent: req.agent,
      model: req.model,
    })

    pruneReplayCache(Date.now())

    const citations = await retrieveFromCollections(req.prompt, req.selectedCollections, 4)
    const systemPrompt = buildPulseSystemPrompt(req, citations)
    const prompt = buildPromptText(req.prompt)

    const contextCharsTotal = computeContextCharsTotal({
      globalClaudeMdChars: GLOBAL_CLAUDE_MD_CHARS,
      systemPromptChars: systemPrompt.length,
      promptLength: req.prompt.length,
      documentMarkdownLength: req.documentMarkdown.length,
      citationSnippets: citations.map((c) => c.snippet),
      threadSources: req.threadSources,
      conversationHistory: req.conversationHistory,
    })

    const encoder = new TextEncoder()
    const cachedReplay = replayCache.get(replayKey)

    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        const replayBuffer = cachedReplay?.events ? [...cachedReplay.events] : []
        let lastEmitAt = Date.now()
        let aborted = request.signal.aborted

        let closed = false
        let terminalHandled = false
        const parserState = createStreamParserState()
        const safeClose = () => {
          if (closed) return
          closed = true
          controller.close()
        }

        const enqueueEvent = (event: PulseChatStreamEvent) => {
          if (closed) return
          lastEmitAt = Date.now()
          try {
            controller.enqueue(encoder.encode(encodePulseChatStreamEvent(event)))
          } catch {
            // Controller was closed externally (client disconnect) — mark closed so no
            // further enqueue attempts are made. The child will be killed via the abort handler.
            closed = true
          }
        }

        const persistReplay = () => {
          upsertReplayEntry(replayKey, replayBuffer, Date.now())
        }

        const emit = (event: Parameters<typeof createPulseChatStreamEvent>[0]) => {
          const normalized = createPulseChatStreamEvent(event)
          replayBuffer.push(normalized)
          if (replayBuffer.length > REPLAY_BUFFER_LIMIT) {
            replayBuffer.shift()
          }
          persistReplay()
          enqueueEvent(normalized)
        }

        const emitErrorAndClose = (error: string, code?: string) => {
          emit({ type: 'error', error, code })
          safeClose()
        }

        const replayFromLastEventId = (): boolean => {
          if (!lastEventId || replayBuffer.length === 0) return false
          const idx = replayBuffer.findIndex((event) => event.event_id === lastEventId)
          if (idx < 0) return false
          const tail = replayBuffer.slice(idx + 1)
          for (const event of tail) {
            enqueueEvent(event)
          }
          return tail.some((event) => event.type === 'done' || event.type === 'error')
        }

        if (replayFromLastEventId()) {
          safeClose()
          return
        }

        emit({ type: 'status', phase: 'started' })

        const abortHandler = () => {
          aborted = true
        }

        request.signal.addEventListener('abort', abortHandler, { once: true })

        const cleanup = () => {
          clearInterval(heartbeatInterval)
          request.signal.removeEventListener('abort', abortHandler)
          persistReplay()
        }

        const heartbeatInterval = setInterval(() => {
          if (closed) return
          if (Date.now() - lastEmitAt < HEARTBEAT_INTERVAL_MS) return
          emit({ type: 'heartbeat', elapsed_ms: Date.now() - startedAt })
        }, HEARTBEAT_INTERVAL_MS)

        const emitMemoryFallbackDone = () => {
          const memoryFallbackText = resolveConversationMemoryAnswer(
            req.prompt,
            req.conversationHistory,
          )
          if (!memoryFallbackText) return false
          const elapsed = Date.now() - startedAt
          emit({
            type: 'done',
            response: {
              text: memoryFallbackText,
              sessionId: parserState.sessionId ?? undefined,
              citations,
              operations: [],
              toolUses: [],
              blocks: [],
              metadata: {
                agent: req.agent,
                model: req.model,
                elapsedMs: elapsed,
                contextCharsTotal,
                contextBudgetChars: MODEL_CONTEXT_BUDGET_CHARS,
                first_delta_ms: parserState.firstDeltaMs,
                time_to_done_ms: elapsed,
                delta_count: parserState.deltaCount,
                aborted,
                fallback_source: 'conversation_memory' as const,
              },
            },
          })
          safeClose()
          return true
        }

        const handlePulsePayload = (payload: unknown) => {
          if (!payload || typeof payload !== 'object' || Array.isArray(payload)) return
          const data = payload as Record<string, unknown>
          const sessionId =
            typeof data.session_id === 'string'
              ? data.session_id
              : typeof data.sessionId === 'string'
                ? data.sessionId
                : null
          if (sessionId) parserState.sessionId = sessionId

          const type = typeof data.type === 'string' ? data.type : ''
          switch (type) {
            case 'status': {
              const phase = data.phase
              if (phase === 'thinking' || phase === 'finalizing') {
                emit({ type: 'status', phase })
              }
              return
            }
            case 'assistant_delta': {
              const delta = typeof data.delta === 'string' ? data.delta : ''
              if (!delta) return
              recordAssistantDelta(parserState, delta, startedAt)
              emit({ type: 'assistant_delta', delta })
              return
            }
            case 'thinking_content': {
              const content =
                typeof data.content === 'string'
                  ? data.content
                  : typeof data.delta === 'string'
                    ? data.delta
                    : ''
              if (!content) return
              recordThinking(parserState, content)
              emit({ type: 'thinking_content', content })
              return
            }
            case 'tool_use': {
              const tool = data.tool
              const toolObj =
                tool && typeof tool === 'object' && !Array.isArray(tool)
                  ? (tool as Record<string, unknown>)
                  : null
              const nameRaw = toolObj?.name ?? data.name ?? data.tool_call_id
              const inputRaw = toolObj?.input ?? data.input
              const name = typeof nameRaw === 'string' && nameRaw.length > 0 ? nameRaw : 'tool'
              const input =
                inputRaw && typeof inputRaw === 'object' && !Array.isArray(inputRaw)
                  ? (inputRaw as Record<string, unknown>)
                  : {}
              const toolUseId =
                typeof data.tool_call_id === 'string' ? data.tool_call_id : undefined
              upsertToolUse(parserState, toolUseId, name, input)
              emit({ type: 'tool_use', tool: { name, input } })
              return
            }
            case 'tool_result': {
              const toolUseId = typeof data.tool_call_id === 'string' ? data.tool_call_id : ''
              if (!toolUseId) return
              const resultText =
                typeof data.result === 'string' ? data.result : extractToolResultText(data.content)
              if (!resultText) return
              patchToolResult(parserState, toolUseId, resultText)
              return
            }
            case 'config_options_update':
            case 'config_option_update': {
              const configOptions = data.configOptions
              if (Array.isArray(configOptions)) {
                emit({ type: 'config_options_update', configOptions })
              }
              return
            }
            case 'result': {
              if (typeof data.result === 'string') {
                parserState.result = data.result
                return
              }
              if (data.result && typeof data.result === 'object') {
                parserState.result = JSON.stringify(data.result)
              }
              return
            }
            default:
              return
          }
        }

        const wsFlags: Record<string, string | boolean> = {}
        if (req.sessionId) {
          wsFlags.session_id = req.sessionId
        }
        wsFlags.agent = req.agent
        if (req.agent === 'claude') {
          wsFlags.model = req.model
        } else if (req.model !== 'default') {
          wsFlags.model = req.model
        }

        void runAxonCommandWsStream('pulse_chat', {
          timeoutMs: resolvePulseChatTimeoutMs(),
          input: prompt,
          flags: wsFlags,
          signal: request.signal,
          onJson: (payload) => {
            handlePulsePayload(payload)
          },
          onDone: ({ exit_code }) => {
            if (terminalHandled || closed) return
            terminalHandled = true
            cleanup()

            if (aborted) {
              emit(
                buildDoneResponse(req, citations, parserState, startedAt, contextCharsTotal, true),
              )
              safeClose()
              return
            }

            if (exit_code !== 0) {
              if (emitMemoryFallbackDone()) return
              emitErrorAndClose(`Pulse chat worker exited ${exit_code}`, 'pulse_chat_exit_nonzero')
              return
            }

            emit({ type: 'status', phase: 'finalizing' })
            emit(
              buildDoneResponse(req, citations, parserState, startedAt, contextCharsTotal, false),
            )
            safeClose()
          },
          onError: ({ message }) => {
            if (terminalHandled || closed) return
            terminalHandled = true
            cleanup()
            if (emitMemoryFallbackDone()) return
            emitErrorAndClose(
              `Pulse chat worker failed: ${truncateForLog(message)}`,
              'pulse_chat_command_error',
            )
          },
        }).catch((error: unknown) => {
          if (terminalHandled || closed) return
          terminalHandled = true
          cleanup()

          if (aborted) {
            emit(buildDoneResponse(req, citations, parserState, startedAt, contextCharsTotal, true))
            safeClose()
            return
          }

          const message = error instanceof Error ? error.message : String(error)
          if (emitMemoryFallbackDone()) return
          emitErrorAndClose(
            `Pulse chat worker transport error: ${truncateForLog(message)}`,
            'pulse_chat_ws',
          )
        })
      },
    })

    return new Response(stream, {
      headers: {
        'content-type': 'application/x-ndjson; charset=utf-8',
        'cache-control': 'no-cache, no-transform',
        connection: 'keep-alive',
      },
    })
  } catch (error: unknown) {
    const errorId = makeErrorId('pulse-chat')
    const message = error instanceof Error ? error.message : String(error)
    console.error('[pulse/chat] unhandled error', { errorId, message, error })
    return apiError(500, 'Chat request failed', { code: 'pulse_chat_internal', errorId })
  }
}

function truncateForLog(input: string, max = 400): string {
  if (input.length <= max) return input
  return `${input.slice(0, max)}...`
}
