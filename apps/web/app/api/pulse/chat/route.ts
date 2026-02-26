import { spawn } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { NextResponse } from 'next/server'
import { fallbackAssistantText, parseClaudeAssistantPayload } from '@/lib/pulse/claude-response'
import { resolveConversationMemoryAnswer } from '@/lib/pulse/conversation-memory'
import { checkPermission } from '@/lib/pulse/permissions'
import { buildPulseSystemPrompt, retrieveFromCollections } from '@/lib/pulse/rag'
import { ensureRepoRootEnvLoaded } from '@/lib/pulse/server-env'
import {
  DocOperationSchema,
  PulseChatRequestSchema,
  type PulseChatResponse,
  type PulseMessageBlock,
  type PulseModel,
  type PulseToolUse,
} from '@/lib/pulse/types'

const CLAUDE_TIMEOUT_MS = 90_000

// The `claude` CLI always injects ~/.claude/CLAUDE.md (global instructions) into every subprocess
// regardless of cwd. Cache the size once at module load so we can include it in context accounting.
let _globalClaudeMdChars = 0
try {
  _globalClaudeMdChars = fs.statSync(path.join(os.homedir(), '.claude', 'CLAUDE.md')).size
} catch {
  // File absent or unreadable — treat as 0.
}
const GLOBAL_CLAUDE_MD_CHARS = _globalClaudeMdChars
const CLAUDE_MODEL_ARG: Record<PulseModel, string> = {
  sonnet: 'sonnet',
  opus: 'opus',
  haiku: 'haiku',
}
// Context budget in chars: 200k token window × ~4 chars/token = 800k chars.
// We measure everything we actually send to the claude subprocess in chars (system prompt,
// CLAUDE.md, user content) and express it as a fraction of this budget.
const MODEL_CONTEXT_BUDGET_CHARS = 800_000

interface ClaudeStreamResult {
  ok: boolean
  error?: string
  result?: string
  session_id?: string
  toolUses: PulseToolUse[]
  blocks: PulseMessageBlock[]
  usage?: {
    input_tokens: number
    output_tokens: number
    cache_read_input_tokens?: number
    cache_creation_input_tokens?: number
  }
}

// Stream-json event shapes (NDJSON, one event per line)
interface ClaudeStreamAssistantContent {
  type: 'text' | 'tool_use'
  text?: string
  id?: string
  name?: string
  input?: Record<string, unknown>
}
interface ClaudeStreamEvent {
  type: 'system' | 'assistant' | 'tool_result' | 'result'
  message?: { content?: ClaudeStreamAssistantContent[] }
  result?: string
  session_id?: string
  subtype?: string
  is_error?: boolean
  // tool_result fields
  tool_use_id?: string
  content?: unknown
  // usage reported in the result event
  usage?: {
    input_tokens: number
    output_tokens: number
    cache_read_input_tokens?: number
    cache_creation_input_tokens?: number
  }
}

function runClaudeStream(args: string[]): Promise<ClaudeStreamResult> {
  return new Promise((resolve) => {
    // Strip CLAUDECODE so the spawned claude CLI doesn't refuse to launch
    // inside an existing Claude Code session.
    const { CLAUDECODE: _cc, ...childEnv } = process.env
    // Use a neutral cwd (not the repo root) so Claude Code doesn't load the
    // axon_rust CLAUDE.md and override the Pulse persona with "I'm a Rust
    // coding assistant."
    const child = spawn('claude', args, {
      cwd: os.tmpdir(),
      env: childEnv,
      stdio: ['ignore', 'pipe', 'pipe'],
    })

    let stdout = ''
    let stderr = ''
    const timer = setTimeout(() => {
      child.kill('SIGTERM')
    }, CLAUDE_TIMEOUT_MS)

    child.stdout.on('data', (chunk: Buffer) => {
      stdout += chunk.toString()
    })
    child.stderr.on('data', (chunk: Buffer) => {
      stderr += chunk.toString()
    })

    child.on('error', (error: Error) => {
      clearTimeout(timer)
      resolve({
        ok: false,
        error: `Failed to start Claude CLI: ${error.message}`,
        toolUses: [],
        blocks: [],
      })
    })

    child.on('close', (code: number | null, signal: NodeJS.Signals | null) => {
      clearTimeout(timer)
      if (signal) {
        resolve({
          ok: false,
          error: `Claude CLI terminated by signal ${signal}`,
          toolUses: [],
          blocks: [],
        })
        return
      }
      if (code !== 0) {
        resolve({
          ok: false,
          error: `Claude CLI exited ${code}: ${truncateForLog(stderr || stdout)}`,
          toolUses: [],
          blocks: [],
        })
        return
      }

      // Parse NDJSON: build ordered blocks (text + tool_use interleaved) + final result
      const toolUses: PulseToolUse[] = []
      const blocks: PulseMessageBlock[] = []
      // Map tool_use id → index in blocks[] for matching tool results
      const toolUseIdToIdx = new Map<string, number>()
      let result = ''
      let session_id: string | undefined
      let usage: ClaudeStreamResult['usage'] | undefined

      for (const line of stdout.split('\n')) {
        const trimmed = line.trim()
        if (!trimmed) continue
        let event: ClaudeStreamEvent
        try {
          event = JSON.parse(trimmed) as ClaudeStreamEvent
        } catch {
          continue
        }

        if (event.type === 'assistant' && event.message?.content) {
          for (const block of event.message.content) {
            if (block.type === 'text' && block.text?.trim()) {
              blocks.push({ type: 'text', content: block.text.trim() })
            }
            if (block.type === 'tool_use' && block.name) {
              const idx = blocks.length
              blocks.push({
                type: 'tool_use',
                name: block.name,
                input: block.input ?? {},
              })
              if (block.id) toolUseIdToIdx.set(block.id, idx)
              toolUses.push({ name: block.name, input: block.input ?? {} })
            }
          }
        }

        if (event.type === 'tool_result') {
          const id = event.tool_use_id
          const raw = event.content
          let resultText = ''
          if (typeof raw === 'string') {
            resultText = raw
          } else if (Array.isArray(raw)) {
            resultText = (raw as Array<unknown>)
              .map((entry) => {
                if (typeof entry !== 'object' || entry === null) return ''
                const obj = entry as Record<string, unknown>
                // direct text block
                if (typeof obj.text === 'string') return obj.text
                // nested content array (tool_result wrapper)
                if (Array.isArray(obj.content)) {
                  return (obj.content as Array<unknown>)
                    .map((inner) => {
                      if (typeof inner !== 'object' || inner === null) return ''
                      const i = inner as Record<string, unknown>
                      return typeof i.text === 'string' ? i.text : ''
                    })
                    .filter(Boolean)
                    .join('\n')
                }
                return ''
              })
              .filter(Boolean)
              .join('\n')
          }
          if (id && resultText) {
            const idx = toolUseIdToIdx.get(id)
            if (idx !== undefined) {
              const b = blocks[idx]
              if (b?.type === 'tool_use') {
                ;(
                  b as {
                    type: 'tool_use'
                    name: string
                    input: Record<string, unknown>
                    result?: string
                  }
                ).result = resultText.slice(0, 600)
              }
            }
          }
        }

        if (event.type === 'result') {
          result = event.result ?? ''
          session_id = event.session_id
          if (event.usage) usage = event.usage
        }
      }

      resolve({ ok: true, result, session_id, toolUses, blocks, usage })
    })
  })
}

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

    const systemPromptChars = systemPrompt.length

    const args = [
      '-p',
      prompt,
      '--output-format',
      'stream-json',
      '--verbose',
      '--system-prompt',
      systemPrompt,
      // Disable all MCP servers — the subprocess runs in a container where
      // none of the globally-configured MCPs are reachable. Without this flag
      // the CLI hangs trying to connect to all servers before answering.
      '--strict-mcp-config',
    ]
    const modelArg = CLAUDE_MODEL_ARG[req.model]
    if (modelArg) {
      args.push('--model', modelArg)
    }
    // Do NOT --resume a Claude Code session. Resuming a session started in a
    // project directory would load that project's CLAUDE.md into Pulse's
    // context (e.g. "you are a Rust coding assistant"). The system prompt
    // built above is the complete context; each request must be fresh.

    const llmResult = await runClaudeStream(args)
    if (!llmResult.ok) {
      const memoryFallbackText = resolveConversationMemoryAnswer(
        req.prompt,
        req.conversationHistory,
      )
      if (memoryFallbackText) {
        const citationChars = citations.reduce(
          (total, citation) => total + citation.snippet.length,
          0,
        )
        const threadSourceChars = req.threadSources.reduce(
          (total, source) => total + source.length,
          0,
        )
        const conversationChars = req.conversationHistory.reduce(
          (total, entry) => total + entry.content.length,
          0,
        )
        const contextCharsTotal =
          GLOBAL_CLAUDE_MD_CHARS +
          systemPromptChars +
          req.prompt.length +
          req.documentMarkdown.length +
          conversationChars +
          citationChars +
          threadSourceChars

        return NextResponse.json({
          text: memoryFallbackText,
          sessionId: undefined,
          citations,
          operations: [],
          toolUses: [],
          blocks: [],
          metadata: {
            model: req.model,
            elapsedMs: Date.now() - startedAt,
            contextCharsTotal,
            contextBudgetChars: MODEL_CONTEXT_BUDGET_CHARS,
          },
        } satisfies PulseChatResponse)
      }
      return NextResponse.json({ error: llmResult.error ?? 'Claude chat failed' }, { status: 502 })
    }

    const raw = llmResult.result ?? ''

    let text = ''
    let operations: PulseChatResponse['operations'] = []
    const parsedPayload = parseClaudeAssistantPayload(raw)
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
      text = fallbackAssistantText(raw)
    }

    const permission = checkPermission(req.permissionLevel, operations, {
      isCurrentDoc: true,
      currentDocMarkdown: req.documentMarkdown,
    })

    if (!permission.allowed) {
      operations = []
      text = text || 'Operation blocked by permission policy.'
    }

    const citationChars = citations.reduce((total, citation) => total + citation.snippet.length, 0)
    const threadSourceChars = req.threadSources.reduce((total, source) => total + source.length, 0)
    const conversationChars = req.conversationHistory.reduce(
      (total, entry) => total + entry.content.length,
      0,
    )
    // Measure everything we actually send to the claude subprocess.
    // GLOBAL_CLAUDE_MD_CHARS: the ~/.claude/CLAUDE.md the CLI always injects.
    // systemPromptChars: our --system-prompt string.
    // The rest: user-supplied content for this request.
    const contextCharsTotal =
      GLOBAL_CLAUDE_MD_CHARS +
      systemPromptChars +
      req.prompt.length +
      req.documentMarkdown.length +
      conversationChars +
      citationChars +
      threadSourceChars

    return NextResponse.json({
      text,
      sessionId: undefined, // session resumption disabled — see --resume comment above
      citations,
      operations,
      toolUses: llmResult.toolUses,
      blocks: llmResult.blocks,
      metadata: {
        model: req.model,
        elapsedMs: Date.now() - startedAt,
        contextCharsTotal,
        contextBudgetChars: MODEL_CONTEXT_BUDGET_CHARS,
      },
    } satisfies PulseChatResponse)
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
