'use client'

import { Brain, ChevronDown, Copy, History, RotateCcw } from 'lucide-react'
import { useState } from 'react'
import type { PulseMessageBlock } from '@/lib/pulse/types'
import type { ChatMessage } from '@/lib/pulse/workspace-persistence'
import { formatMessageTime } from './chat-utils'
import { DocOpBadge } from './doc-op-badge'
import { PulseMarkdown } from './pulse-markdown'
import { type BadgeTool, ToolCallBadge } from './tool-badge'

// ── Thinking block (collapsible reasoning display) ────────────────────────────

function ThinkingBlock({ content }: { content: string }) {
  const [open, setOpen] = useState(false)
  return (
    <div className="rounded-lg border border-[rgba(167,139,250,0.2)] bg-[rgba(15,5,30,0.4)]">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-left"
      >
        <Brain className="size-3 shrink-0 text-violet-400" />
        <span className="text-[length:var(--text-xs)] font-medium text-violet-300">Reasoning</span>
        <span className="ml-auto text-[length:var(--text-2xs)] text-[var(--axon-text-dim)]">
          {open ? 'hide' : `${content.length} chars`}
        </span>
        <ChevronDown
          className={`size-3 text-violet-300 transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </button>
      {open && (
        <div className="border-t border-[rgba(167,139,250,0.15)] px-2.5 py-2">
          <p className="whitespace-pre-wrap font-mono text-[length:var(--text-xs)] leading-relaxed text-[var(--axon-text-secondary)]">
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
        <div className="flex items-center gap-1.5 text-[length:var(--text-xs)] text-[var(--axon-text-dim)]">
          <History className="size-3 shrink-0 text-[var(--axon-accent-blue)]" />
          <span>{handoffLabel}</span>
        </div>
      )
    }
  }

  const hasStructuredBlocks =
    msg.blocks?.some((b) => b.type === 'tool_use' || b.type === 'thinking') ?? false
  if (hasStructuredBlocks && msg.blocks) {
    const groups = groupBlocksForRender(msg.blocks)
    return (
      <div className="space-y-1.5">
        {groups.map((group, i) => {
          if (group.kind === 'thinking') {
            return <ThinkingBlock key={i} content={group.content} />
          }
          if (group.kind === 'text') {
            // Prefer msg.content (parsed clean text set after completion) over
            // group.content, which may contain the raw JSON wrapper Claude uses
            // to encode document operations alongside the response text.
            const displayContent =
              msg.role === 'assistant' && msg.content ? msg.content : group.content
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
  onRetry: (prompt: string) => void
  copyStatus: 'idle' | 'copied' | 'failed'
  onCopyError: (content: string) => void
}

export function MessageBubble({ msg, onRetry, copyStatus, onCopyError }: MessageBubbleProps) {
  const isUser = msg.role === 'user'
  return (
    <article className="w-full space-y-1.5">
      <div className={`flex w-full ${isUser ? 'justify-end' : 'justify-start'}`}>
        <div
          className={`rounded-xl border px-3 py-2.5 shadow-[0_6px_18px_rgba(3,7,18,0.3)] ${
            isUser
              ? 'max-w-[86%] border-[rgba(175,215,255,0.26)] bg-[linear-gradient(140deg,rgba(175,215,255,0.2),rgba(175,215,255,0.08))] text-[var(--axon-text-primary)] md:max-w-[78%] lg:max-w-[70%]'
              : 'max-w-[92%] border-[rgba(255,135,175,0.18)] bg-[linear-gradient(140deg,rgba(255,135,175,0.1),rgba(10,18,35,0.55))] text-[var(--axon-text-secondary)] md:max-w-[86%] lg:max-w-[78%]'
          }`}
        >
          {/* Header: role label + timestamp */}
          <div className="mb-1.5 flex items-center justify-between gap-2">
            <span
              className={`inline-flex items-center gap-1 text-[length:var(--text-2xs)] font-semibold uppercase tracking-[0.1em] ${
                isUser
                  ? 'text-[var(--axon-accent-pink-strong)]'
                  : 'text-[var(--axon-accent-blue-strong)]'
              }`}
            >
              <span
                className={`inline-block size-1.5 rounded-full ${
                  isUser ? 'bg-[var(--axon-accent-pink)]' : 'bg-[var(--axon-accent-blue)]'
                }`}
              />
              {isUser ? 'You' : 'Cortex'}
            </span>
            <span className="text-[length:var(--text-2xs)] text-[var(--axon-text-dim)]">
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
                  onClick={() => onCopyError(msg.content)}
                  className="ui-chip inline-flex items-center gap-1 rounded border border-[rgba(95,135,175,0.3)] bg-[rgba(10,18,35,0.44)] px-2 py-1 text-[var(--axon-text-dim)]"
                >
                  <Copy className="size-3" />
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
      </div>
    </article>
  )
}
