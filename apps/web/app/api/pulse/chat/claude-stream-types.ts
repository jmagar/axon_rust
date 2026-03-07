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

export const CLAUDE_MODEL_ARG: Record<string, string> = {
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

export interface ClaudeBuildExtra {
  effort?: string
  maxTurns?: number
  maxBudgetUsd?: number
  appendSystemPrompt?: string
  // Additional CLI flags from Claude Code docs
  disableSlashCommands?: boolean
  noSessionPersistence?: boolean
  fallbackModel?: string
  allowedTools?: string
  disallowedTools?: string
  addDir?: string
  betas?: string
  toolsRestrict?: string
}

const DEFAULT_ALLOWED_BETAS = new Set(['interleaved-thinking'])
const envAllowedBetas = (process.env.AXON_ALLOWED_CLAUDE_BETAS ?? '')
  .split(',')
  .map((value) => value.trim())
  .filter(Boolean)
const ALLOWED_CLAUDE_BETAS =
  envAllowedBetas.length > 0 ? new Set(envAllowedBetas) : DEFAULT_ALLOWED_BETAS

function sanitizeBetas(raw: string): string {
  return raw
    .split(',')
    .map((value) => value.trim())
    .filter((value) => value.length > 0 && ALLOWED_CLAUDE_BETAS.has(value))
    .join(',')
}

// Allowed root directories for --add-dir. Defaults are container paths;
// override with PULSE_ALLOWED_DIR_ROOTS (comma-separated) for local dev.
const ALLOWED_DIR_ROOTS = process.env.PULSE_ALLOWED_DIR_ROOTS
  ? process.env.PULSE_ALLOWED_DIR_ROOTS.split(',')
      .map((d) => d.trim())
      .filter(Boolean)
  : ['/home/node', '/tmp', '/workspace']

function validateAddDir(dir: string): string | null {
  // Resolve the path first, then follow symlinks so a symlink inside an allowed
  // root (e.g. /tmp/evil → /etc) cannot bypass the allowlist check.
  let real: string
  try {
    real = fs.realpathSync(path.resolve(dir))
  } catch {
    // Path does not exist yet — fall back to lexical resolution.
    // Non-existent paths cannot be symlinks, so path.resolve is safe here.
    real = path.resolve(dir)
  }
  if (ALLOWED_DIR_ROOTS.some((root) => real.startsWith(root + path.sep) || real === root)) {
    return real
  }
  return null
}

/**
 * Resolve whether --dangerously-skip-permissions should be passed to the Claude CLI.
 *
 * Checks AXON_ALLOW_SKIP_PERMISSIONS first (preferred), then falls back to the legacy
 * PULSE_SKIP_PERMISSIONS env var. Both must be explicitly set to 'true' to enable.
 * Any other value (including unset) disables the flag.
 */
function resolveSkipPermissions(): boolean {
  const explicit = process.env.AXON_ALLOW_SKIP_PERMISSIONS
  if (explicit !== undefined) {
    return explicit === 'true'
  }
  // Legacy fallback: PULSE_SKIP_PERMISSIONS defaulted to enabled (anything != 'false').
  // Preserve that behavior for existing deployments that haven't migrated.
  return process.env.PULSE_SKIP_PERMISSIONS !== 'false'
}

export function buildClaudeArgs(
  prompt: string,
  systemPrompt: string,
  model: PulseModel,
  extra?: ClaudeBuildExtra,
): string[] {
  const args = [
    '-p',
    prompt,
    '--output-format',
    'stream-json',
    '--verbose',
    '--system-prompt',
    systemPrompt,
    // Load MCPs exclusively from the project-owned config file.
    // --strict-mcp-config ensures ~/.claude.json MCPs are ignored entirely —
    // only what's in mcp.json is loaded, preventing hangs on unreachable servers.
    '--mcp-config',
    process.env.CLAUDE_MCP_CONFIG ?? '/home/node/.claude/mcp.json',
    '--strict-mcp-config',
    // SECURITY: --dangerously-skip-permissions bypasses all tool permission checks.
    // Only enable when the adapter runs in a non-interactive context (no TTY) AND the
    // operator has explicitly opted in via AXON_ALLOW_SKIP_PERMISSIONS=true.
    // Legacy env var PULSE_SKIP_PERMISSIONS is respected as a fallback.
    ...(resolveSkipPermissions() ? ['--dangerously-skip-permissions'] : []),
    // Stream partial tool inputs and thinking blocks as they arrive.
    // Requires -p + stream-json (both already set above).
    '--include-partial-messages',
    // Calibrate inference effort for document-grounded Q&A.
    '--effort',
    extra?.effort ?? 'medium',
    // Explicit plugin dir inside the project-owned ~/.claude mount.
    '--plugin-dir',
    process.env.CLAUDE_PLUGIN_DIR ?? '/home/node/.claude/plugins',
  ]
  const modelArg = CLAUDE_MODEL_ARG[model] ?? model
  if (modelArg) {
    args.push('--model', modelArg)
  }
  if (extra?.appendSystemPrompt) {
    args.push('--append-system-prompt', extra.appendSystemPrompt)
  }
  if (extra?.maxTurns && extra.maxTurns > 0) {
    args.push('--max-turns', String(extra.maxTurns))
  }
  if (extra?.maxBudgetUsd && extra.maxBudgetUsd > 0) {
    args.push('--max-budget-usd', String(extra.maxBudgetUsd))
  }
  if (extra?.disableSlashCommands) {
    args.push('--disable-slash-commands')
  }
  if (extra?.noSessionPersistence) {
    args.push('--no-session-persistence')
  }
  if (extra?.fallbackModel) {
    args.push('--fallback-model', extra.fallbackModel)
  }
  // Allow valid tool identifiers: letters, digits, underscore, wildcards, parens (e.g. Bash(*))
  const TOOL_ENTRY_RE = /^[a-zA-Z][a-zA-Z0-9_*(),:]*$/
  if (extra?.allowedTools) {
    const filtered = extra.allowedTools
      .split(',')
      .map((t) => t.trim())
      .filter((t) => TOOL_ENTRY_RE.test(t))
      .join(',')
    if (filtered) {
      args.push('--allowedTools', filtered)
    }
  }
  if (extra?.disallowedTools) {
    const filtered = extra.disallowedTools
      .split(',')
      .map((t) => t.trim())
      .filter((t) => TOOL_ENTRY_RE.test(t))
      .join(',')
    if (filtered) {
      args.push('--disallowedTools', filtered)
    }
  }
  if (extra?.addDir) {
    for (const dir of extra.addDir.split(',')) {
      const trimmed = dir.trim()
      const validated = trimmed ? validateAddDir(trimmed) : null
      if (validated) {
        args.push('--add-dir', validated)
      }
    }
  }
  if (extra?.betas) {
    const betas = sanitizeBetas(extra.betas)
    if (betas) {
      args.push('--betas', betas)
    }
  }
  if (extra?.toolsRestrict) {
    const filtered = extra.toolsRestrict
      .split(',')
      .map((t) => t.trim())
      .filter((t) => TOOL_ENTRY_RE.test(t))
      .join(',')
    if (filtered) {
      args.push('--tools', filtered)
    }
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
