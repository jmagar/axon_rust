import type { createPulseChatStreamEvent } from '@/lib/pulse/chat-stream'
import { fallbackAssistantText, parseClaudeAssistantPayload } from '@/lib/pulse/claude-response'
import { checkPermission } from '@/lib/pulse/permissions'
import type { PulseChatRequest, PulseCitation } from '@/lib/pulse/types'
import { DocOperationSchema, type PulseChatResponse } from '@/lib/pulse/types'
import { MODEL_CONTEXT_BUDGET_CHARS } from './claude-stream-types'
import type { createStreamParserState } from './stream-parser'

// ── Prompt / response building ──────────────────────────────────────────────

export function buildPromptText(userPrompt: string): string {
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

type ParseOperationsResult = { text: string; operations: PulseChatResponse['operations'] }

export function parseOperations(result: string): ParseOperationsResult {
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

export function buildDoneResponse(
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

// ── Parser state mutation helpers ───────────────────────────────────────────

export function recordAssistantDelta(
  parserState: ReturnType<typeof createStreamParserState>,
  delta: string,
  startedAt: number,
): void {
  parserState.blocks.push({ type: 'text', content: delta })
  parserState.deltaCount += 1
  parserState.firstDeltaMs ??= Date.now() - startedAt
}

export function recordThinking(
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

export function upsertToolUse(
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

export function patchToolResult(
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

export function truncateForLog(input: string, max = 400): string {
  return input.length <= max ? input : `${input.slice(0, max)}...`
}
