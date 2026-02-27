import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import type { PulseModel } from '@/lib/pulse/types'

export const CLAUDE_TIMEOUT_MS = 300_000 // 5 min — agentic research tasks need room to breathe

// The `claude` CLI always injects ~/.claude/CLAUDE.md (global instructions) into every subprocess
// regardless of cwd. Cache the size once at module load so we can include it in context accounting.
let _globalClaudeMdChars = 0
try {
  _globalClaudeMdChars = fs.statSync(path.join(os.homedir(), '.claude', 'CLAUDE.md')).size
} catch {
  // File absent or unreadable — treat as 0.
}
export const GLOBAL_CLAUDE_MD_CHARS = _globalClaudeMdChars

export const CLAUDE_MODEL_ARG: Record<PulseModel, string> = {
  sonnet: 'sonnet',
  opus: 'opus',
  haiku: 'haiku',
}

// Context budget in chars: 200k token window × ~4 chars/token = 800k chars.
// We measure everything we actually send to the claude subprocess in chars (system prompt,
// CLAUDE.md, user content) and express it as a fraction of this budget.
export const MODEL_CONTEXT_BUDGET_CHARS = 800_000

export const HEARTBEAT_INTERVAL_MS = 5_000

// Stream-json event shapes (NDJSON, one event per line)
export interface ClaudeStreamAssistantContent {
  type: 'text' | 'tool_use' | 'thinking'
  text?: string
  thinking?: string
  id?: string
  name?: string
  input?: Record<string, unknown>
}

export interface ClaudeStreamEvent {
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

export function buildClaudeArgs(prompt: string, systemPrompt: string, model: PulseModel): string[] {
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
    // No TTY in the container — skip all interactive permission prompts.
    '--dangerously-skip-permissions',
    // Stream partial tool inputs and thinking blocks as they arrive.
    // Requires -p + stream-json (both already set above).
    '--include-partial-messages',
    // Calibrate inference effort for document-grounded Q&A.
    '--effort',
    'medium',
    // Explicit plugin dir inside the project-owned ~/.claude mount.
    '--plugin-dir',
    '/home/node/.claude/plugins',
  ]
  const modelArg = CLAUDE_MODEL_ARG[model]
  if (modelArg) {
    args.push('--model', modelArg)
  }
  return args
}

export function computeContextCharsTotal(params: {
  globalClaudeMdChars: number
  systemPromptChars: number
  promptLength: number
  documentMarkdownLength: number
  citationSnippets: string[]
  threadSources: string[]
  conversationHistory: Array<{ content: string }>
}): number {
  const citationChars = params.citationSnippets.reduce(
    (total, snippet) => total + snippet.length,
    0,
  )
  const threadSourceChars = params.threadSources.reduce((total, source) => total + source.length, 0)
  const conversationChars = params.conversationHistory.reduce(
    (total, entry) => total + entry.content.length,
    0,
  )
  return (
    params.globalClaudeMdChars +
    params.systemPromptChars +
    params.promptLength +
    params.documentMarkdownLength +
    conversationChars +
    citationChars +
    threadSourceChars
  )
}
