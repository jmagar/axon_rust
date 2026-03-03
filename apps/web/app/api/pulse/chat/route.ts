import { spawn } from 'node:child_process'
import os from 'node:os'
import { NextResponse } from 'next/server'
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
  PulseChatRequestSchema,
  type PulseChatResponse,
  type PulseToolUse,
} from '@/lib/pulse/types'
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
} from './replay-cache'
import { createStreamParserState, parseClaudeStreamLine } from './stream-parser'

export async function POST(request: Request) {
  ensureRepoRootEnvLoaded()
  const startedAt = Date.now()

  let body: unknown
  try {
    body = await request.json()
  } catch {
    return NextResponse.json({ error: 'Request body must be valid JSON' }, { status: 400 })
  }

  try {
    const parsed = PulseChatRequestSchema.safeParse(body)
    if (!parsed.success) {
      return NextResponse.json(
        { error: parsed.error.issues[0]?.message ?? 'Invalid request payload' },
        { status: 400 },
      )
    }

    const req = parsed.data
    const bodyObject =
      typeof body === 'object' && body !== null ? (body as Record<string, unknown>) : {}
    const lastEventId =
      typeof bodyObject.last_event_id === 'string'
        ? bodyObject.last_event_id
        : typeof bodyObject.lastEventId === 'string'
          ? bodyObject.lastEventId
          : undefined

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
    const prompt = [
      req.prompt,
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
          replayCache.set(replayKey, { events: replayBuffer, updatedAt: Date.now() })
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

        const buildTelemetry = () => {
          const elapsed = Date.now() - startedAt
          return {
            elapsedMs: elapsed,
            contextCharsTotal,
            contextBudgetChars: MODEL_CONTEXT_BUDGET_CHARS,
            first_delta_ms: parserState.firstDeltaMs,
            time_to_done_ms: elapsed,
            delta_count: parserState.deltaCount,
            aborted,
          }
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

        // Strip CLAUDECODE so the spawned claude CLI doesn't refuse to launch
        // inside an existing Claude Code session.
        const { CLAUDECODE: _cc, ...childEnv } = process.env
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
          if (closed) return
          closed = true
          cleanup()
          emitErrorAndClose(`Failed to start Claude CLI: ${error.message}`, 'pulse_chat_spawn')
        })

        child.on('close', (code: number | null, signal: NodeJS.Signals | null) => {
          if (closed) return
          closed = true
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

          const toolUses: PulseToolUse[] = parserState.toolUses
          const blocks = parserState.blocks
          const result = parserState.result

          if (aborted) {
            emit({
              type: 'done',
              response: {
                text: fallbackAssistantText(result),
                sessionId: parserState.sessionId ?? undefined,
                citations,
                operations: [],
                toolUses,
                blocks,
                metadata: {
                  model: req.model,
                  ...buildTelemetry(),
                },
              },
            })
            safeClose()
            return
          }

          if (code !== 0) {
            // Prefer the result from the stream-json parser (auth errors, tool errors) over
            // stderr (usually empty for non-zero exits) or raw stdout remainder.
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
                    ...buildTelemetry(),
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

          let text = ''
          let operations: PulseChatResponse['operations'] = []
          const parsedPayload = parseClaudeAssistantPayload(result)
          if (parsedPayload) {
            text = parsedPayload.text
            if (parsedPayload.operations.length > 0) {
              const parsedOps: PulseChatResponse['operations'] = []
              for (const op of parsedPayload.operations) {
                const parsedOp = DocOperationSchema.safeParse(op)
                if (parsedOp.success) {
                  parsedOps.push(parsedOp.data)
                }
              }
              operations = parsedOps
            }
          } else {
            text = fallbackAssistantText(result)
          }

          const permission = checkPermission(req.permissionLevel, operations, {
            isCurrentDoc: true,
            currentDocMarkdown: req.documentMarkdown,
          })

          if (!permission.allowed) {
            operations = []
            text = text || 'Operation blocked by permission policy.'
          }

          emit({
            type: 'done',
            response: {
              text,
              sessionId: parserState.sessionId ?? undefined,
              citations,
              operations,
              toolUses,
              blocks,
              metadata: {
                model: req.model,
                ...buildTelemetry(),
              },
            },
          })
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
    const errorId = globalThis.crypto?.randomUUID?.() ?? `pulse-chat-${Date.now()}`
    const message = error instanceof Error ? error.message : String(error)
    console.error('[pulse/chat] unhandled error', { errorId, message, error })
    return NextResponse.json(
      { error: 'Chat request failed', code: 'pulse_chat_internal', errorId },
      { status: 500 },
    )
  }
}

function truncateForLog(input: string, max = 400): string {
  if (input.length <= max) return input
  return `${input.slice(0, max)}...`
}
