'use client'

import { ChevronDown, Copy, MessageSquare, PenLine, RotateCcw, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { PulseMessageBlock, PulseToolUse } from '@/lib/pulse/types'
import { PulseMarkdown } from './pulse-markdown'
import type { ChatMessage } from './pulse-workspace'

// ── Tool-use style helpers ─────────────────────────────────────────────────────

type BadgeStyle = { dot: string; border: string; bg: string; label: string }

function toolBadgeStyle(name: string): BadgeStyle {
  if (['Read', 'Write', 'Edit', 'Glob', 'Grep', 'LS'].includes(name)) {
    return {
      dot: 'bg-[var(--axon-accent-blue)]',
      border: 'border-[rgba(175,215,255,0.22)]',
      bg: 'bg-[rgba(15,23,42,0.55)]',
      label: 'text-[var(--axon-accent-blue)]',
    }
  }
  if (name === 'Bash') {
    return {
      dot: 'bg-amber-400',
      border: 'border-[rgba(245,158,11,0.28)]',
      bg: 'bg-[rgba(30,20,5,0.55)]',
      label: 'text-amber-300',
    }
  }
  if (['WebFetch', 'WebSearch'].includes(name)) {
    return {
      dot: 'bg-teal-400',
      border: 'border-[rgba(45,212,191,0.28)]',
      bg: 'bg-[rgba(5,20,20,0.55)]',
      label: 'text-teal-300',
    }
  }
  if (name === 'Task' || name.includes(':')) {
    return {
      dot: 'bg-[var(--axon-accent-pink)]',
      border: 'border-[rgba(255,135,175,0.28)]',
      bg: 'bg-[rgba(20,5,15,0.55)]',
      label: 'text-[var(--axon-accent-pink)]',
    }
  }
  return {
    dot: 'bg-emerald-400',
    border: 'border-[rgba(52,211,153,0.24)]',
    bg: 'bg-[rgba(5,20,10,0.55)]',
    label: 'text-emerald-300',
  }
}

function toolInputSummary(use: PulseToolUse): string {
  const { name, input } = use
  const str = (v: unknown) => (typeof v === 'string' ? v : '')
  if (name === 'Read' || name === 'Write' || name === 'Edit') {
    const p = str(input.file_path)
    return p ? (p.split('/').pop() ?? p) : ''
  }
  if (name === 'Bash') return str(input.command).slice(0, 52)
  if (name === 'Glob') return str(input.pattern).slice(0, 44)
  if (name === 'Grep') return str(input.pattern).slice(0, 44)
  if (name === 'WebFetch' || name === 'WebSearch') {
    const raw = str(input.url ?? input.query)
    try {
      return new URL(raw).hostname
    } catch {
      return raw.slice(0, 40)
    }
  }
  if (name === 'Task') return str(input.description).slice(0, 52)
  if (name === 'LS') return str(input.path).split('/').pop() ?? str(input.path)
  return ''
}

// ── Expandable tool call card ──────────────────────────────────────────────────

type ToolUseBlock = Extract<PulseMessageBlock, { type: 'tool_use' }>

function ToolCallBlock({ block }: { block: ToolUseBlock }) {
  const [expanded, setExpanded] = useState(false)
  const style = toolBadgeStyle(block.name)
  const summary = toolInputSummary({ name: block.name, input: block.input })

  return (
    <div className={`rounded-lg border ${style.border} ${style.bg} text-[length:var(--text-xs)]`}>
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-1.5 px-2 py-1.5 text-left"
      >
        <span className={`size-1.5 shrink-0 rounded-full ${style.dot}`} />
        <span className={`font-semibold ${style.label}`}>{block.name}</span>
        {summary && (
          <span className="min-w-0 flex-1 truncate text-[var(--axon-text-dim)]">· {summary}</span>
        )}
        <ChevronDown
          className={`ml-auto size-3 shrink-0 text-[var(--axon-text-dim)] transition-transform duration-150 ${
            expanded ? 'rotate-180' : ''
          }`}
        />
      </button>

      {expanded && (
        <div className="border-t border-[rgba(255,255,255,0.06)] px-2 pb-2 pt-1.5 space-y-2">
          <div>
            <p className="ui-label mb-1">Input</p>
            <pre className="max-h-[140px] overflow-y-auto whitespace-pre-wrap break-all rounded bg-[rgba(0,0,0,0.25)] p-1.5 text-[length:var(--text-xs)] leading-[var(--leading-copy)] text-[var(--axon-text-secondary)]">
              {JSON.stringify(block.input, null, 2)}
            </pre>
          </div>
          {block.result && (
            <div>
              <p className="ui-label mb-1">Result</p>
              <pre className="max-h-[100px] overflow-y-auto whitespace-pre-wrap break-all rounded bg-[rgba(0,0,0,0.25)] p-1.5 text-[length:var(--text-xs)] leading-[var(--leading-copy)] text-[var(--axon-text-dim)]">
                {block.result}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

// ── Doc-op pill (operations are post-processed, not inline in stream) ──────────

function DocOpBadge({ type, heading }: { type: string; heading?: string }) {
  const labels: Record<string, string> = {
    replace_document: 'Replace doc',
    append_markdown: 'Append',
    insert_section: heading ? `Insert · ${heading}` : 'Insert section',
  }
  const label = labels[type] ?? type
  return (
    <span className="ui-chip inline-flex shrink-0 items-center gap-1 rounded-full border border-[rgba(52,211,153,0.24)] bg-[rgba(5,20,10,0.55)] px-2 py-0.5">
      <span className="inline-block size-1.5 shrink-0 rounded-full bg-emerald-400" />
      <span className="font-medium text-emerald-300">{label}</span>
    </span>
  )
}

// ── Message content renderer ───────────────────────────────────────────────────

function MessageContent({ msg }: { msg: ChatMessage }) {
  if (msg.isError) return null

  // Only use block rendering when there are tool_use blocks; otherwise the
  // blocks array contains Claude's raw JSON response text and we should
  // render the already-parsed msg.content instead.
  const hasToolCalls = msg.blocks?.some((b) => b.type === 'tool_use') ?? false
  if (hasToolCalls && msg.blocks) {
    return (
      <div className="space-y-1.5">
        {msg.blocks.map((block, i) => {
          if (block.type === 'text') {
            return msg.role === 'assistant' ? (
              <PulseMarkdown key={i} content={block.content} />
            ) : (
              <p key={i} className="ui-copy whitespace-pre-wrap">
                {block.content}
              </p>
            )
          }
          return <ToolCallBlock key={i} block={block} />
        })}
      </div>
    )
  }

  if (msg.role === 'assistant') {
    return <PulseMarkdown content={msg.content} />
  }
  return <p className="ui-copy whitespace-pre-wrap">{msg.content}</p>
}

// ── Main component ─────────────────────────────────────────────────────────────

interface PulseChatPaneProps {
  messages: ChatMessage[]
  isLoading: boolean
  indexedSources: string[]
  activeThreadSources: string[]
  onRemoveSource: (url: string) => void
  onRetry: (prompt: string) => void
  mobilePane: 'chat' | 'editor'
  onMobilePaneChange: (pane: 'chat' | 'editor') => void
  isDesktop: boolean
  requestNotice?: string | null
}

const CHAT_SCROLL_STORAGE_KEY = 'axon.web.pulse.chat-scroll'
const SOURCE_LIST_SCROLL_STORAGE_KEY = 'axon.web.pulse.source-list-scroll'
const SOURCE_EXPANDED_STORAGE_KEY = 'axon.web.pulse.sources-expanded'
const SOURCE_LIST_OPEN_STORAGE_KEY = 'axon.web.pulse.sources-open'
const MESSAGE_VIRTUAL_THRESHOLD = 120
const MESSAGE_ESTIMATED_HEIGHT = 156
const MESSAGE_OVERSCAN = 8
const SOURCE_VIRTUAL_THRESHOLD = 40
const SOURCE_ROW_HEIGHT = 24
const SOURCE_OVERSCAN = 6

export function computeMessageVirtualWindow(
  totalMessages: number,
  scrollOffset: number,
  viewportPx: number,
): { shouldVirtualize: boolean; start: number; end: number } {
  const shouldVirtualize = totalMessages > MESSAGE_VIRTUAL_THRESHOLD
  if (!shouldVirtualize) {
    return { shouldVirtualize: false, start: 0, end: totalMessages }
  }
  const start = Math.max(0, Math.floor(scrollOffset / MESSAGE_ESTIMATED_HEIGHT) - MESSAGE_OVERSCAN)
  const visibleCount =
    Math.ceil(Math.max(viewportPx, 1) / MESSAGE_ESTIMATED_HEIGHT) + MESSAGE_OVERSCAN * 2
  const end = Math.min(totalMessages, start + visibleCount)
  return { shouldVirtualize: true, start, end }
}

function formatMessageTime(createdAt: number | undefined): string {
  if (!createdAt) return ''
  return new Date(createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

export function PulseChatPane({
  messages,
  isLoading,
  indexedSources,
  activeThreadSources,
  onRemoveSource,
  onRetry,
  mobilePane,
  onMobilePaneChange,
  isDesktop,
  requestNotice,
}: PulseChatPaneProps) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const sourceListRef = useRef<HTMLDivElement>(null)
  const [isNearBottom, setIsNearBottom] = useState(true)
  const [showJumpToLatest, setShowJumpToLatest] = useState(false)
  const [sourcesExpanded, setSourcesExpanded] = useState(false)
  const [sourceListOpen, setSourceListOpen] = useState(false)
  const [copyStatus, setCopyStatus] = useState<'idle' | 'copied' | 'failed'>('idle')
  const [sourceListScrollTop, setSourceListScrollTop] = useState(0)
  const [scrollTop, setScrollTop] = useState(0)
  const [viewportHeight, setViewportHeight] = useState(0)
  const activeSources = useMemo(() => activeThreadSources, [activeThreadSources])
  const latestAssistantCitations = useMemo(() => {
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      const msg = messages[i]
      if (msg.role === 'assistant' && msg.citations && msg.citations.length > 0) {
        return msg.citations
      }
    }
    return []
  }, [messages])
  const visibleSources = useMemo(() => activeSources.slice(0, 4), [activeSources])
  const hiddenSourceCount = Math.max(0, activeSources.length - visibleSources.length)
  const messageWindow = computeMessageVirtualWindow(messages.length, scrollTop, viewportHeight)
  const shouldVirtualizeMessages = messageWindow.shouldVirtualize
  const virtualStartIndex = messageWindow.start
  const virtualEndIndex = messageWindow.end
  const virtualMessages = shouldVirtualizeMessages
    ? messages.slice(virtualStartIndex, virtualEndIndex)
    : messages
  const topSpacerHeight = shouldVirtualizeMessages
    ? virtualStartIndex * MESSAGE_ESTIMATED_HEIGHT
    : 0
  const bottomSpacerHeight = shouldVirtualizeMessages
    ? Math.max(0, (messages.length - virtualEndIndex) * MESSAGE_ESTIMATED_HEIGHT)
    : 0
  const shouldVirtualizeSources = sourceListOpen && activeSources.length > SOURCE_VIRTUAL_THRESHOLD
  const sourceVirtualStart = shouldVirtualizeSources
    ? Math.max(0, Math.floor(sourceListScrollTop / SOURCE_ROW_HEIGHT) - SOURCE_OVERSCAN)
    : 0
  const sourceVirtualCount = shouldVirtualizeSources
    ? 18 + SOURCE_OVERSCAN * 2
    : activeSources.length
  const sourceVirtualEnd = shouldVirtualizeSources
    ? Math.min(activeSources.length, sourceVirtualStart + sourceVirtualCount)
    : activeSources.length
  const visibleThreadSources = sourceListOpen
    ? shouldVirtualizeSources
      ? activeSources.slice(sourceVirtualStart, sourceVirtualEnd)
      : activeSources
    : visibleSources
  const sourceTopSpacer = shouldVirtualizeSources ? sourceVirtualStart * SOURCE_ROW_HEIGHT : 0
  const sourceBottomSpacer = shouldVirtualizeSources
    ? Math.max(0, (activeSources.length - sourceVirtualEnd) * SOURCE_ROW_HEIGHT)
    : 0
  function scrollToBottom() {
    const node = scrollRef.current
    if (!node) return
    node.scrollTop = node.scrollHeight
    setShowJumpToLatest(false)
  }

  useEffect(() => {
    const node = scrollRef.current
    if (!node) return
    setViewportHeight(node.clientHeight)
    try {
      const saved = Number(window.localStorage.getItem(CHAT_SCROLL_STORAGE_KEY) ?? 0)
      if (Number.isFinite(saved) && saved > 0) {
        node.scrollTop = saved
        setScrollTop(saved)
        const nearBottom = node.scrollHeight - (saved + node.clientHeight) < 42
        setIsNearBottom(nearBottom)
        setShowJumpToLatest(!nearBottom && messages.length > 0)
      }
    } catch {
      // Ignore storage restore failures.
    }
  }, [])

  useEffect(() => {
    try {
      const expanded = window.localStorage.getItem(SOURCE_EXPANDED_STORAGE_KEY)
      const open = window.localStorage.getItem(SOURCE_LIST_OPEN_STORAGE_KEY)
      if (expanded === '1' || expanded === '0') setSourcesExpanded(expanded === '1')
      if (open === '1' || open === '0') setSourceListOpen(open === '1')
    } catch {
      // Ignore storage restore failures.
    }
  }, [])

  useEffect(() => {
    if (!sourceListOpen) return
    const node = sourceListRef.current
    if (!node) return
    try {
      const saved = Number(window.localStorage.getItem(SOURCE_LIST_SCROLL_STORAGE_KEY) ?? 0)
      if (Number.isFinite(saved) && saved > 0) {
        node.scrollTop = saved
        setSourceListScrollTop(saved)
      }
    } catch {
      // Ignore storage restore failures.
    }
  }, [sourceListOpen])

  useEffect(() => {
    try {
      window.localStorage.setItem(SOURCE_EXPANDED_STORAGE_KEY, sourcesExpanded ? '1' : '0')
    } catch {
      // Ignore storage write failures.
    }
  }, [sourcesExpanded])

  useEffect(() => {
    try {
      window.localStorage.setItem(SOURCE_LIST_OPEN_STORAGE_KEY, sourceListOpen ? '1' : '0')
    } catch {
      // Ignore storage write failures.
    }
  }, [sourceListOpen])

  useEffect(() => {
    const node = scrollRef.current
    if (!node) return
    if (isNearBottom) {
      node.scrollTop = node.scrollHeight
      setShowJumpToLatest(false)
    } else if (messages.length > 0) {
      setShowJumpToLatest(true)
    }
  }, [isLoading, isNearBottom, messages])

  async function handleCopyError(content: string) {
    try {
      const canUseClipboard =
        typeof navigator !== 'undefined' &&
        typeof navigator.clipboard !== 'undefined' &&
        typeof navigator.clipboard.writeText === 'function'

      if (canUseClipboard) {
        await navigator.clipboard.writeText(content)
      } else if (typeof document !== 'undefined') {
        const textArea = document.createElement('textarea')
        textArea.value = content
        textArea.style.position = 'fixed'
        textArea.style.left = '-9999px'
        document.body.appendChild(textArea)
        textArea.focus()
        textArea.select()
        const copied = document.execCommand('copy')
        document.body.removeChild(textArea)
        if (!copied) throw new Error('copy_failed')
      } else {
        throw new Error('clipboard_unavailable')
      }
      setCopyStatus('copied')
      setTimeout(() => setCopyStatus('idle'), 1200)
    } catch {
      setCopyStatus('failed')
      setTimeout(() => setCopyStatus('idle'), 1400)
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-b border-[rgba(255,135,175,0.1)] bg-[linear-gradient(120deg,rgba(175,215,255,0.05),rgba(255,135,175,0.03))] px-3 py-2">
        <div className="flex items-center gap-1.5">
          <p className="ui-label flex-none">Pulse Chat</p>

          {/* Mobile pane switcher */}
          {!isDesktop && (
            <>
              <button
                type="button"
                onClick={() => onMobilePaneChange('chat')}
                aria-pressed={mobilePane === 'chat'}
                title="Chat pane"
                className={`inline-flex size-6 items-center justify-center rounded border transition-colors ${
                  mobilePane === 'chat'
                    ? 'border-[rgba(175,215,255,0.35)] bg-[rgba(175,215,255,0.18)] text-[var(--axon-accent-pink-strong)]'
                    : 'border-[rgba(255,135,175,0.16)] bg-[rgba(10,18,35,0.42)] text-[var(--axon-text-dim)]'
                }`}
              >
                <MessageSquare className="size-3" />
              </button>
              <button
                type="button"
                onClick={() => onMobilePaneChange('editor')}
                aria-pressed={mobilePane === 'editor'}
                title="Editor pane"
                className={`inline-flex size-6 items-center justify-center rounded border transition-colors ${
                  mobilePane === 'editor'
                    ? 'border-[rgba(175,215,255,0.35)] bg-[rgba(175,215,255,0.18)] text-[var(--axon-accent-pink-strong)]'
                    : 'border-[rgba(255,135,175,0.16)] bg-[rgba(10,18,35,0.42)] text-[var(--axon-text-dim)]'
                }`}
              >
                <PenLine className="size-3" />
              </button>
            </>
          )}

          <div className="min-w-0 flex-1" />

          {/* Sources dropdown */}

          <button
            type="button"
            onClick={() => setSourcesExpanded((prev) => !prev)}
            className="ui-chip inline-flex flex-none items-center gap-1 rounded border border-[rgba(95,135,175,0.24)] bg-[rgba(10,18,35,0.45)] px-1.5 py-0.5 text-[var(--axon-text-subtle)]"
            aria-expanded={sourcesExpanded}
            title={sourcesExpanded ? 'Hide sources' : 'Show sources'}
          >
            {Math.max(activeSources.length, latestAssistantCitations.length)} src
            <ChevronDown
              className={`size-3 transition-transform ${sourcesExpanded ? 'rotate-180' : ''}`}
            />
          </button>
        </div>
        {requestNotice && (
          <div className="mt-1 rounded border border-[rgba(255,192,134,0.3)] bg-[rgba(255,192,134,0.08)] px-1.5 py-1 ui-meta text-[var(--axon-warning)]">
            {requestNotice}
          </div>
        )}
        {sourcesExpanded && (activeSources.length > 0 || latestAssistantCitations.length > 0) && (
          <div className="mt-1.5 space-y-1.5">
            {activeSources.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {visibleSources.map((source) => (
                  <span
                    key={source}
                    className="inline-flex max-w-[190px] items-center gap-1 truncate rounded-full border border-[rgba(95,135,175,0.28)] bg-[rgba(15,23,42,0.48)] px-1.5 py-0.5 text-[length:var(--text-2xs)] text-[var(--axon-text-dim)]"
                    title={source}
                  >
                    {source}
                    <button
                      type="button"
                      onClick={() => onRemoveSource(source)}
                      aria-label={`Remove ${source} from thread context`}
                      className="inline-flex items-center justify-center rounded-full text-[var(--axon-text-subtle)] transition-colors hover:text-[var(--axon-text-primary)]"
                      title="Remove from this thread context"
                    >
                      <X className="size-2.5" />
                    </button>
                  </span>
                ))}
                {hiddenSourceCount > 0 && (
                  <button
                    type="button"
                    onClick={() => setSourceListOpen((prev) => !prev)}
                    className="ui-chip inline-flex items-center rounded-full border border-[rgba(255,135,175,0.2)] bg-[rgba(10,18,35,0.42)] px-1.5 py-0.5 text-[var(--axon-text-subtle)]"
                    aria-expanded={sourceListOpen}
                  >
                    +{hiddenSourceCount} more
                  </button>
                )}
                {indexedSources.length > activeSources.length && (
                  <span className="ui-chip inline-flex items-center rounded-full border border-[rgba(255,135,175,0.2)] bg-[rgba(10,18,35,0.42)] px-1.5 py-0.5 text-[var(--axon-text-subtle)]">
                    {indexedSources.length - activeSources.length} inactive
                  </span>
                )}
              </div>
            )}
            {latestAssistantCitations.length > 0 && (
              <div className="rounded-md border border-[rgba(95,135,175,0.22)] bg-[rgba(10,18,35,0.45)] p-1.5">
                <p className="ui-label mb-1">Response sources</p>
                <div className="space-y-1">
                  {latestAssistantCitations.slice(0, 4).map((citation, citationIndex) => (
                    <a
                      key={`${citation.url}-${citationIndex}`}
                      href={citation.url}
                      target="_blank"
                      rel="noreferrer"
                      className="block rounded border border-[rgba(255,135,175,0.15)] bg-[rgba(10,18,35,0.45)] px-1.5 py-1 transition-colors hover:border-[rgba(175,215,255,0.38)]"
                    >
                      <div className="mb-0.5 flex items-center justify-between gap-1">
                        <p className="line-clamp-1 text-[length:var(--text-xs)] font-semibold text-[var(--axon-accent-blue)]">
                          {citation.title}
                        </p>
                        <span className="ui-chip rounded border border-[rgba(175,215,255,0.24)] bg-[rgba(175,215,255,0.1)] px-1 py-0.5 text-[var(--axon-text-dim)]">
                          {citation.collection}
                        </span>
                      </div>
                      <p className="line-clamp-2 text-[length:var(--text-xs)] leading-[var(--leading-tight)] text-[var(--axon-text-dim)]">
                        {citation.snippet}
                      </p>
                    </a>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
        {sourceListOpen && activeSources.length > 0 && (
          <div
            ref={sourceListRef}
            onScroll={() => {
              try {
                if (!sourceListRef.current) return
                setSourceListScrollTop(sourceListRef.current.scrollTop)
                window.localStorage.setItem(
                  SOURCE_LIST_SCROLL_STORAGE_KEY,
                  String(sourceListRef.current.scrollTop),
                )
              } catch {
                // Ignore storage failures.
              }
            }}
            className="mt-1.5 max-h-24 space-y-1 overflow-y-auto rounded-md border border-[rgba(95,135,175,0.22)] bg-[rgba(10,18,35,0.45)] p-1.5"
          >
            {sourceTopSpacer > 0 && <div style={{ height: `${sourceTopSpacer}px` }} aria-hidden />}
            {visibleThreadSources.map((source) => (
              <div key={`full-${source}`} className="flex items-center justify-between gap-2">
                <span className="truncate text-[length:var(--text-2xs)] text-[var(--axon-text-dim)]">
                  {source}
                </span>
                <button
                  type="button"
                  onClick={() => onRemoveSource(source)}
                  className="inline-flex items-center rounded border border-[rgba(95,135,175,0.25)] px-1 py-0.5 text-[length:var(--text-2xs)] text-[var(--axon-text-subtle)]"
                >
                  Remove
                </button>
              </div>
            ))}
            {sourceBottomSpacer > 0 && (
              <div style={{ height: `${sourceBottomSpacer}px` }} aria-hidden />
            )}
          </div>
        )}
      </div>

      <div
        ref={scrollRef}
        onScroll={() => {
          const node = scrollRef.current
          if (!node) return
          setScrollTop(node.scrollTop)
          setViewportHeight(node.clientHeight)
          const nearBottom = node.scrollHeight - (node.scrollTop + node.clientHeight) < 42
          setIsNearBottom(nearBottom)
          if (nearBottom) setShowJumpToLatest(false)
          try {
            window.localStorage.setItem(CHAT_SCROLL_STORAGE_KEY, String(node.scrollTop))
          } catch {
            // Ignore storage failures.
          }
        }}
        className="flex min-h-0 flex-1 flex-col space-y-2.5 overflow-y-auto px-3 py-2.5"
      >
        {messages.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <div className="max-w-sm rounded-xl border border-[rgba(255,135,175,0.12)] bg-[rgba(10,18,35,0.5)] p-3 text-center">
              <p className="text-[length:var(--text-md)] font-medium text-[var(--axon-text-secondary)]">
                Start the thread from omnibox.
              </p>
              <p className="ui-meta mt-0.5">
                Ask a question to chat with Claude, or paste a URL to run the selected command.
              </p>
            </div>
          </div>
        ) : (
          <>
            {topSpacerHeight > 0 && <div style={{ height: `${topSpacerHeight}px` }} aria-hidden />}
            {virtualMessages.map((msg, index) => {
              const absoluteIndex = virtualStartIndex + index
              const isUser = msg.role === 'user'
              return (
                <article
                  key={msg.id ?? `legacy-${absoluteIndex}-${msg.role}-${msg.content.slice(0, 24)}`}
                  className="w-full space-y-1.5"
                >
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
                              isUser
                                ? 'bg-[var(--axon-accent-pink)]'
                                : 'bg-[var(--axon-accent-blue)]'
                            }`}
                          />
                          {isUser ? 'You' : 'Claude'}
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
                              onClick={() => {
                                void handleCopyError(msg.content)
                              }}
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
            })}
            {bottomSpacerHeight > 0 && (
              <div style={{ height: `${bottomSpacerHeight}px` }} aria-hidden />
            )}
          </>
        )}

        {isLoading && (
          <div className="flex items-center gap-2 px-1 text-[length:var(--text-xs)] text-[var(--axon-text-dim)]">
            <span className="inline-block size-1.5 animate-pulse rounded-full bg-[var(--axon-accent-pink)]" />
            Claude is thinking...
          </div>
        )}

        {showJumpToLatest && (
          <button
            type="button"
            onClick={scrollToBottom}
            className="ui-chip sticky bottom-2 ml-auto inline-flex items-center rounded-full border border-[rgba(175,215,255,0.28)] bg-[rgba(10,18,35,0.72)] px-2 py-1 text-[var(--axon-text-dim)] shadow-[0_4px_12px_rgba(3,7,18,0.32)]"
          >
            Jump to latest
          </button>
        )}
      </div>
    </div>
  )
}
