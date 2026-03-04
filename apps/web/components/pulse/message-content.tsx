'use client'

import { Brain, Check, ChevronDown, Copy, History, RotateCcw } from 'lucide-react'
import { memo, useState } from 'react'
import { parseClaudeAssistantPayload } from '@/lib/pulse/claude-response'
import type { PulseMessageBlock } from '@/lib/pulse/types'
import type { ChatMessage } from '@/lib/pulse/workspace-persistence'
import { formatMessageTime } from './chat-utils'
import { DocOpBadge } from './doc-op-badge'
import { PulseMarkdown } from './pulse-markdown'
import { type BadgeTool, ToolCallBadge } from './tool-badge'

// ── Thinking block (collapsible reasoning display) ────────────────────────────

function ThinkingBlock({ content }: { content: string }) {
  const [open, setOpen] = useState(false)
  const wordCount = content.trim().split(/\s+/).filter(Boolean).length
  return (
    <div className="rounded-lg border border-[rgba(167,139,250,0.2)] bg-[rgba(15,5,30,0.4)]">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-left hover:bg-[rgba(167,139,250,0.08)] transition-colors rounded-t-lg"
      >
        <Brain className="size-3 shrink-0 text-violet-400" />
        <span className="text-[length:var(--text-xs)] font-medium text-violet-300">Reasoning</span>
        <span className="ml-auto text-[length:var(--text-2xs)] text-[var(--text-dim)]">
          {open ? 'hide' : `${wordCount} ${wordCount === 1 ? 'word' : 'words'}`}
        </span>
        <ChevronDown
          className={`size-3 text-violet-300 transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </button>
      {open && (
        <div className="border-t border-[rgba(167,139,250,0.15)] px-2.5 py-2 animate-fade-in">
          <p className="whitespace-pre-wrap font-mono text-[length:var(--text-xs)] leading-relaxed text-[var(--text-secondary)]">
            {content}
          </p>
        </div>
      )}
    </div>
  )
}

// ── Block grouping (collapse consecutive tool calls into badge rows) ───────────

type RenderGroup =
  | { kind: 'text'; content: string }
  | { kind: 'thinking'; content: string }
  | { kind: 'tools'; tools: BadgeTool[] }

export function groupBlocksForRender(blocks: PulseMessageBlock[]): RenderGroup[] {
  const result: RenderGroup[] = []
  let toolBatch: BadgeTool[] = []

  for (const block of blocks) {
    if (block.type === 'tool_use') {
      toolBatch.push({ name: block.name, input: block.input, result: block.result })
    } else if (block.type === 'thinking') {
      if (toolBatch.length > 0) {
        result.push({ kind: 'tools', tools: toolBatch })
        toolBatch = []
      }
      result.push({ kind: 'thinking', content: block.content })
    } else if (block.type === 'text') {
      if (toolBatch.length > 0) {
        result.push({ kind: 'tools', tools: toolBatch })
        toolBatch = []
      }
      result.push({ kind: 'text', content: block.content })
    }
  }
  if (toolBatch.length > 0) result.push({ kind: 'tools', tools: toolBatch })
  return result
}

// ── Session handoff chip ───────────────────────────────────────────────────────

const HANDOFF_PREFIX = "I'm loading a previous Claude Code session from project:"

function parseHandoffLabel(content: string): string | null {
  if (!content.startsWith(HANDOFF_PREFIX)) return null
  // Extract project name: "...from project: **foo**..."
  const match = /from project: \*\*(.+?)\*\*/.exec(content)
  const project = match?.[1] ?? 'unknown'
  const turnCount = (content.match(/### (USER|ASSISTANT):/g) ?? []).length
  return `Loaded session: ${project} · ${turnCount} turns`
}

// ── Message content renderer ───────────────────────────────────────────────────

export function MessageContent({ msg }: { msg: ChatMessage }) {
  if (msg.isError) return null

  // Compact chip for session handoff messages
  if (msg.role === 'user') {
    const handoffLabel = parseHandoffLabel(msg.content)
    if (handoffLabel) {
      return (
        <div className="flex items-center gap-1.5 text-[length:var(--text-xs)] text-[var(--text-dim)]">
          <History className="size-3 shrink-0 text-[var(--axon-secondary)]" />
          <span>{handoffLabel}</span>
        </div>
      )
    }
  }

  const hasStructuredBlocks =
    msg.blocks?.some((b) => b.type === 'tool_use' || b.type === 'thinking') ?? false
  if (hasStructuredBlocks && msg.blocks) {
    const groups = groupBlocksForRender(msg.blocks)
    // Count text groups so we know when msg.content is safe to use as a substitute.
    // msg.content is the full accumulated response text — using it for every text
    // group duplicates the full response when there are multiple text segments.
    // Only substitute msg.content when there is exactly one text group.
    const textGroupCount = groups.filter((g) => g.kind === 'text').length
    return (
      <div className="space-y-1.5">
        {groups.map((group, i) => {
          if (group.kind === 'thinking') {
            return <ThinkingBlock key={i} content={group.content} />
          }
          if (group.kind === 'text') {
            // Prefer msg.content (parsed clean text set after completion) over
            // group.content only when there is a single text group — otherwise
            // msg.content (the full response) would be repeated for each segment.
            // When group.content looks like raw Claude JSON, strip it to avoid
            // showing the JSON wrapper from streaming deltas.
            const parsedText = parseClaudeAssistantPayload(group.content)?.text
            const rawGroupContent =
              parsedText != null && parsedText !== '' ? parsedText : group.content
            const displayContent =
              msg.role === 'assistant' && msg.content && textGroupCount === 1
                ? msg.content
                : rawGroupContent
            return msg.role === 'assistant' ? (
              <PulseMarkdown key={i} content={displayContent} />
            ) : (
              <p key={i} className="ui-copy whitespace-pre-wrap">
                {group.content}
              </p>
            )
          }
          return (
            <div key={i} className="flex flex-wrap gap-1">
              {group.tools.map((tool, j) => (
                <ToolCallBadge key={`${tool.name}-${j}`} tool={tool} />
              ))}
            </div>
          )
        })}
      </div>
    )
  }

  if (msg.role === 'assistant') {
    return <PulseMarkdown content={msg.content} />
  }
  return <p className="ui-copy whitespace-pre-wrap">{msg.content}</p>
}

// ── Message bubble (full per-message card with header, content, doc-ops) ──────

interface MessageBubbleProps {
  msg: ChatMessage
  index: number
  onRetry: (prompt: string) => void
  copyStatus: 'idle' | 'copied' | 'failed'
  onCopyError: (content: string) => void
}

export const MessageBubble = memo(function MessageBubble({
  msg,
  index,
  onRetry,
  copyStatus,
  onCopyError,
}: MessageBubbleProps) {
  const isUser = msg.role === 'user'
  const [copyAnim, setCopyAnim] = useState(false)
  return (
    <div className={`flex w-full ${isUser ? 'justify-end' : 'justify-start'}`}>
      <article
        className={`w-full space-y-1.5 animate-fade-in-up ${
          isUser ? 'mr-4 max-w-[80%]' : 'ml-2 max-w-[80%]'
        }`}
        style={{ animationDelay: `${Math.min(index * 25, 150)}ms` }}
      >
        <div
          className={`rounded-xl border px-3 py-2.5 ${
            isUser
              ? 'border-[var(--border-standard)] bg-[linear-gradient(140deg,rgba(135,175,255,0.28),rgba(135,175,255,0.12))] shadow-[var(--shadow-md)] text-[var(--text-primary)]'
              : 'border-[rgba(255,135,175,0.18)] bg-[linear-gradient(140deg,rgba(255,135,175,0.1),rgba(10,18,35,0.55))] shadow-[0_6px_18px_rgba(3,7,18,0.3)] text-[var(--text-secondary)]'
          }`}
        >
          {/* Header: role label + timestamp */}
          <div className="mb-1.5 flex items-center justify-between gap-2">
            <span
              className={`inline-flex items-center gap-1 text-[length:var(--text-2xs)] font-semibold uppercase tracking-[0.1em] ${
                isUser ? 'text-[var(--axon-primary)]' : 'text-[var(--axon-secondary-strong)]'
              }`}
            >
              <span
                className={`inline-block size-1.5 rounded-full ${
                  isUser ? 'bg-[var(--axon-primary-strong)]' : 'bg-[var(--axon-secondary)]'
                }`}
              />
              {isUser ? 'You' : 'Cortex'}
            </span>
            <span className="text-[11px] text-[var(--text-muted)] font-medium">
              {formatMessageTime(msg.createdAt)}
            </span>
          </div>

          {/* Error state */}
          {msg.isError ? (
            <div className="space-y-2">
              <p className="ui-copy whitespace-pre-wrap text-rose-200">{msg.content}</p>
              <div className="flex items-center gap-1.5">
                <button
                  type="button"
                  onClick={() => msg.retryPrompt && onRetry(msg.retryPrompt)}
                  disabled={!msg.retryPrompt}
                  className="ui-chip inline-flex items-center gap-1 rounded border border-[rgba(255,135,135,0.3)] bg-[rgba(127,29,29,0.28)] px-2 py-1 text-rose-200"
                >
                  <RotateCcw className="size-3" />
                  Retry
                </button>
                <button
                  type="button"
                  onClick={() => {
                    onCopyError(msg.content)
                    setCopyAnim(true)
                    setTimeout(() => setCopyAnim(false), 1400)
                  }}
                  className={`ui-chip inline-flex items-center gap-1 rounded border px-2 py-1 transition-all duration-200 ${
                    copyStatus === 'copied'
                      ? 'border-[rgba(130,217,160,0.4)] bg-[rgba(130,217,160,0.12)] text-[var(--axon-success)]'
                      : copyStatus === 'failed'
                        ? 'border-[rgba(255,135,175,0.4)] bg-[rgba(255,135,175,0.08)] text-[var(--axon-secondary)]'
                        : 'border-[rgba(95,135,175,0.3)] bg-[var(--surface-float)] text-[var(--text-dim)]'
                  }`}
                >
                  {copyStatus === 'copied' ? (
                    <Check className={`size-3 ${copyAnim ? 'animate-check-bounce' : ''}`} />
                  ) : (
                    <Copy className="size-3" />
                  )}
                  {copyStatus === 'copied'
                    ? 'Copied'
                    : copyStatus === 'failed'
                      ? 'Copy failed'
                      : 'Copy'}
                </button>
              </div>
            </div>
          ) : (
            /* Inline blocks (text + tool calls in order) or plain text */
            <MessageContent msg={msg} />
          )}

          {/* Doc-op pills — post-processed operations, shown after content */}
          {!isUser && (msg.operations?.length ?? 0) > 0 && (
            <div className="mt-2 flex flex-wrap gap-1 border-t border-[rgba(255,255,255,0.06)] pt-2">
              {msg.operations?.map((op, i) => (
                <DocOpBadge
                  key={i}
                  type={op.type}
                  heading={'heading' in op ? op.heading : undefined}
                />
              ))}
            </div>
          )}
        </div>
      </article>
    </div>
  )
})
