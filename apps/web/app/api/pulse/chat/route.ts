import { spawn } from 'node:child_process'
import os from 'node:os'
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
  buildClaudeArgs,
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
import { createStreamParserState, parseClaudeStreamLine } from './stream-parser'

// ── Request preparation (extracted from POST for readability) ────────────────

function buildPromptText(userPrompt: string): string {
  return [
    userPrompt,
    '',
    'Respond as JSON only with this exact shape:',
    '{"text":"...","operations":[...]}',
    'Allowed operation types and their required fields:',
    '  replace_document: {"type":"replace_document","markdown":"<full doc content>"}',
    '  append_markdown:  {"type":"append_markdown","markdown":"<content to append>"}',
    '  insert_section:   {"type":"insert_section","heading":"<title>","markdown":"<content>","position":"top"|"bottom"}',
    'IMPORTANT: use "markdown" (not "content") for the document text field.',
    'If no operations are needed, return operations as an empty array.',
  ].join('\n')
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
        metadata: { model: req.model, ...telemetry },
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
      metadata: { model: req.model, ...telemetry },
    },
  }
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
      model: req.model,
    })

    pruneReplayCache(Date.now())

    const citations = await retrieveFromCollections(req.prompt, req.selectedCollections, 4)
    const systemPrompt = buildPulseSystemPrompt(req, citations)
    const prompt = buildPromptText(req.prompt)

    const args = buildClaudeArgs(prompt, systemPrompt, req.model, {
      effort: req.effort,
      maxTurns: req.maxTurns,
      maxBudgetUsd: req.maxBudgetUsd,
      appendSystemPrompt: req.appendSystemPrompt,
      disableSlashCommands: req.disableSlashCommands,
      noSessionPersistence: req.noSessionPersistence,
      fallbackModel: req.fallbackModel,
      allowedTools: req.allowedTools,
      disallowedTools: req.disallowedTools,
      addDir: req.addDir,
      betas: req.betas,
      toolsRestrict: req.toolsRestrict,
    })
    // Resume the previous Claude Code session when the client supplies one.
    // Safe because cwd is always os.tmpdir() — no project CLAUDE.md is loaded.
    if (req.sessionId) {
      args.push('--resume', req.sessionId)
    }

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
        let childHandled = false
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

        // Build an explicit environment allowlist for the claude CLI child process.
        // The child must NOT inherit server secrets (AXON_PG_URL, OPENAI_API_KEY,
        // TAVILY_API_KEY, REDDIT_CLIENT_SECRET, AXON_WEB_API_TOKEN, etc.).
        // Only pass variables the CLI legitimately needs to operate.
        const CLAUDE_CHILD_ENV_ALLOWLIST = new Set([
          'PATH',
          'HOME',
          'USER',
          'SHELL',
          'TERM',
          'LANG',
          'LC_ALL',
          'NODE_ENV',
          'AXON_WORKSPACE',
          'TMPDIR',
          'TMP',
          'TEMP',
          'XDG_RUNTIME_DIR',
          'DBUS_SESSION_BUS_ADDRESS',
        ])
        const childEnv = Object.fromEntries(
          Object.entries(process.env).filter(([key]) => CLAUDE_CHILD_ENV_ALLOWLIST.has(key)),
        ) as NodeJS.ProcessEnv
        const child = spawn('claude', args, {
          cwd: process.env.AXON_WORKSPACE ?? os.tmpdir(),
          env: childEnv,
          stdio: ['ignore', 'pipe', 'pipe'],
        })

        let stderr = ''
        let stdoutRemainder = ''
        const parserState = createStreamParserState()

        const abortHandler = () => {
          aborted = true
          if (!closed) {
            child.kill('SIGTERM')
          }
        }

        request.signal.addEventListener('abort', abortHandler, { once: true })

        const cleanup = () => {
          clearTimeout(timer)
          clearInterval(heartbeatInterval)
          request.signal.removeEventListener('abort', abortHandler)
          persistReplay()
        }

        const timer = setTimeout(() => {
          child.kill('SIGTERM')
        }, CLAUDE_TIMEOUT_MS)

        const heartbeatInterval = setInterval(() => {
          if (closed) return
          if (Date.now() - lastEmitAt < HEARTBEAT_INTERVAL_MS) return
          emit({ type: 'heartbeat', elapsed_ms: Date.now() - startedAt })
        }, HEARTBEAT_INTERVAL_MS)

        child.stdout.on('data', (chunk: Buffer) => {
          const chunkText = chunk.toString()
          const combined = stdoutRemainder + chunkText
          const lines = combined.split('\n')
          stdoutRemainder = lines.pop() ?? ''

          for (const line of lines) {
            const result = parseClaudeStreamLine(line, parserState, startedAt)
            if (result.kind === 'skip') continue
            if (result.kind === 'result') continue // stored in parserState.result
            if (result.kind === 'tool_result_patch') continue // already patched in parserState.blocks
            if (result.kind === 'assistant_events') {
              for (const ev of result.events) {
                emit(ev)
              }
            }
          }
        })

        child.stderr.on('data', (chunk: Buffer) => {
          if (stderr.length < 16_384) stderr += chunk.toString()
        })

        child.on('error', (error: Error) => {
          if (childHandled || closed) return
          childHandled = true
          cleanup()
          emitErrorAndClose(`Failed to start Claude CLI: ${error.message}`, 'pulse_chat_spawn')
        })

        child.on('close', (code: number | null, signal: NodeJS.Signals | null) => {
          if (childHandled || closed) return
          childHandled = true
          cleanup()

          // Flush any partial line that didn't end with a newline (e.g. the final `result` event).
          if (stdoutRemainder.trim()) {
            const flushResult = parseClaudeStreamLine(stdoutRemainder, parserState, startedAt)
            stdoutRemainder = ''
            if (flushResult.kind === 'assistant_events') {
              for (const ev of flushResult.events) emit(ev)
            }
          }

          if (signal && !aborted) {
            emitErrorAndClose(
              `Claude CLI terminated by signal ${signal}`,
              'pulse_chat_terminated_signal',
            )
            return
          }

          if (aborted) {
            emit(buildDoneResponse(req, citations, parserState, startedAt, contextCharsTotal, true))
            safeClose()
            return
          }

          if (code !== 0) {
            const cliErrorDetail = parserState.result || truncateForLog(stderr || stdoutRemainder)
            console.error('[pulse/chat] Claude CLI exited', code, {
              stderr: (stderr || '').slice(0, 500),
              result: parserState.result.slice(0, 500),
            })
            const memoryFallbackText = resolveConversationMemoryAnswer(
              req.prompt,
              req.conversationHistory,
            )
            if (memoryFallbackText) {
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
              return
            }
            emitErrorAndClose(
              `Claude CLI exited ${code}: ${cliErrorDetail}`,
              'pulse_chat_exit_nonzero',
            )
            return
          }

          emit({ type: 'status', phase: 'finalizing' })
          emit(buildDoneResponse(req, citations, parserState, startedAt, contextCharsTotal, false))
          safeClose()
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
