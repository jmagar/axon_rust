import type { PulseMessageBlock, PulseToolUse } from '@/lib/pulse/types'
import type { ClaudeStreamAssistantContent, ClaudeStreamEvent } from './claude-stream-types'

export type StreamParserState = {
  blocks: PulseMessageBlock[]
  toolUseIdToIdx: Map<string, number>
  toolUses: PulseToolUse[]
  result: string
  sessionId: string | null
  firstDeltaMs: number | null
  deltaCount: number
}

export function createStreamParserState(): StreamParserState {
  return {
    blocks: [],
    toolUseIdToIdx: new Map<string, number>(),
    toolUses: [],
    result: '',
    sessionId: null,
    firstDeltaMs: null,
    deltaCount: 0,
  }
}

// Pure function — never throws. Handles all malformed shapes by returning ''.
export function extractToolResultText(raw: unknown): string {
  try {
    if (typeof raw === 'string') return raw
    if (!Array.isArray(raw)) return ''
    return (raw as Array<unknown>)
      .map((entry) => {
        if (typeof entry !== 'object' || entry === null) return ''
        const obj = entry as Record<string, unknown>
        if (typeof obj.text === 'string') return obj.text
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
  } catch {
    return ''
  }
}

export type ParsedLineResult =
  | {
      kind: 'assistant_events'
      events: Array<
        | { type: 'status'; phase: 'thinking' }
        | { type: 'assistant_delta'; delta: string }
        | { type: 'tool_use'; tool: PulseToolUse }
        | { type: 'thinking_content'; content: string }
      >
    }
  | { kind: 'result'; result: string }
  | { kind: 'tool_result_patch'; toolUseId: string; resultText: string }
  | { kind: 'skip' }

// Parses one NDJSON line from the claude --output-format stream-json output.
// Mutates state.blocks, state.toolUseIdToIdx, state.toolUses, state.result,
// state.firstDeltaMs, and state.deltaCount as a side effect.
// Returns { kind: 'skip' } for unknown or malformed lines; never throws.
export function parseClaudeStreamLine(
  line: string,
  state: StreamParserState,
  startedAt: number,
): ParsedLineResult {
  const trimmed = line.trim()
  if (!trimmed) return { kind: 'skip' }

  let event: ClaudeStreamEvent
  try {
    event = JSON.parse(trimmed) as ClaudeStreamEvent
  } catch {
    return { kind: 'skip' }
  }

  if (event.type === 'assistant' && event.message?.content) {
    const outEvents: Array<
      | { type: 'status'; phase: 'thinking' }
      | { type: 'assistant_delta'; delta: string }
      | { type: 'tool_use'; tool: PulseToolUse }
      | { type: 'thinking_content'; content: string }
    > = [{ type: 'status', phase: 'thinking' }]

    for (const block of event.message.content as ClaudeStreamAssistantContent[]) {
      if (block.type === 'text' && block.text) {
        state.blocks.push({ type: 'text', content: block.text })
        state.deltaCount += 1
        if (state.firstDeltaMs === null) {
          state.firstDeltaMs = Date.now() - startedAt
        }
        outEvents.push({ type: 'assistant_delta', delta: block.text })
      }
      if (block.type === 'tool_use' && block.name) {
        const existingIdx = block.id ? state.toolUseIdToIdx.get(block.id) : undefined
        if (existingIdx !== undefined) {
          // Partial-message update: same tool ID seen again with more complete input.
          // Update the existing block in-place instead of creating a duplicate.
          const existingBlock = state.blocks[existingIdx]
          if (existingBlock?.type === 'tool_use') {
            existingBlock.input = block.input ?? {}
          }
        } else {
          const tool: PulseToolUse = {
            name: block.name,
            input: block.input ?? {},
          }
          const idx = state.blocks.length
          state.blocks.push({
            type: 'tool_use',
            name: block.name,
            input: block.input ?? {},
          })
          if (block.id) state.toolUseIdToIdx.set(block.id, idx)
          state.toolUses.push(tool)
          outEvents.push({ type: 'tool_use', tool })
        }
      }
      if (block.type === 'thinking' && block.thinking) {
        // Partial-message update: growing thinking block — update in-place if the
        // previous block is already a thinking block to avoid duplicate Reasoning boxes.
        const lastBlock = state.blocks[state.blocks.length - 1]
        if (lastBlock?.type === 'thinking') {
          lastBlock.content = block.thinking
        } else {
          state.blocks.push({ type: 'thinking', content: block.thinking })
        }
        outEvents.push({ type: 'thinking_content', content: block.thinking })
      }
    }

    return { kind: 'assistant_events', events: outEvents }
  }

  if (event.type === 'tool_result') {
    const id = event.tool_use_id
    const resultText = extractToolResultText(event.content)

    if (id && resultText) {
      const idx = state.toolUseIdToIdx.get(id)
      if (idx !== undefined) {
        const b = state.blocks[idx]
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
      return { kind: 'tool_result_patch', toolUseId: id, resultText }
    }

    return { kind: 'skip' }
  }

  if (event.type === 'result') {
    state.result = event.result ?? ''
    state.sessionId = event.session_id ?? null
    return { kind: 'result', result: state.result }
  }

  return { kind: 'skip' }
}
