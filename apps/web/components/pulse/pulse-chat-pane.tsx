'use client'

import { MessageCircle, Send, Square, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { PulseToolUse } from '@/lib/pulse/types'
import type { ChatMessage } from '@/lib/pulse/workspace-persistence'
import {
  CHAT_SCROLL_STORAGE_KEY,
  computeMessageVirtualWindow,
  formatStreamPhaseLabel,
  MESSAGE_ESTIMATED_HEIGHT,
  SOURCE_LIST_OPEN_STORAGE_KEY,
  SOURCE_LIST_SCROLL_STORAGE_KEY,
  SOURCE_OVERSCAN,
  SOURCE_ROW_HEIGHT,
  SOURCE_VIRTUAL_THRESHOLD,
} from './chat-utils'
import { MessageBubble } from './message-content'

// ── Main component ─────────────────────────────────────────────────────────────
interface PulseChatPaneProps {
  messages: ChatMessage[]
  isLoading: boolean
  streamingPhase?: 'started' | 'thinking' | 'finalizing' | null
  liveToolUses?: PulseToolUse[]
  onCancelRequest?: () => void
  indexedSources: string[]
  activeThreadSources: string[]
  onRemoveSource: (url: string) => void
  onRetry: (prompt: string) => void
  sourcesExpanded: boolean
  onSourcesExpandedChange: (expanded: boolean) => void
  requestNotice?: string | null
}

export function PulseChatPane({
  messages,
  isLoading,
  streamingPhase,
  liveToolUses: _liveToolUses = [],
  onCancelRequest,
  indexedSources,
  activeThreadSources,
  onRemoveSource,
  onRetry,
  sourcesExpanded,
  onSourcesExpandedChange: _onSourcesExpandedChange,
  requestNotice,
}: PulseChatPaneProps) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const sourceListRef = useRef<HTMLDivElement>(null)
  const [isNearBottom, setIsNearBottom] = useState(true)
  const [showJumpToLatest, setShowJumpToLatest] = useState(false)
  const [sourceListOpen, setSourceListOpen] = useState(false)
  const [copyStatuses, setCopyStatuses] = useState<Map<string, 'idle' | 'copied' | 'failed'>>(
    new Map(),
  )
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
  }, [messages.length])

  useEffect(() => {
    try {
      const open = window.localStorage.getItem(SOURCE_LIST_OPEN_STORAGE_KEY)
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
  }, [isNearBottom, messages])

  async function handleCopyError(content: string, messageId: string) {
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
      setCopyStatuses((prev) => new Map(prev).set(messageId, 'copied'))
      setTimeout(() => {
        setCopyStatuses((prev) => {
          const next = new Map(prev)
          next.delete(messageId)
          return next
        })
      }, 1200)
    } catch {
      setCopyStatuses((prev) => new Map(prev).set(messageId, 'failed'))
      setTimeout(() => {
        setCopyStatuses((prev) => {
          const next = new Map(prev)
          next.delete(messageId)
          return next
        })
      }, 1400)
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      {(requestNotice ||
        (sourcesExpanded && (activeSources.length > 0 || latestAssistantCitations.length > 0)) ||
        (sourceListOpen && activeSources.length > 0)) && (
        <div className="border-b border-[rgba(255,135,175,0.1)] bg-[linear-gradient(120deg,rgba(175,215,255,0.05),rgba(255,135,175,0.03))] px-3 py-2">
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
                      {sourceListOpen ? 'Hide sources' : `+${hiddenSourceCount} more`}
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
            <div className="animate-slide-down overflow-hidden">
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
                {sourceTopSpacer > 0 && (
                  <div style={{ height: `${sourceTopSpacer}px` }} aria-hidden />
                )}
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
            </div>
          )}
        </div>
      )}

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
          <div className="flex h-full items-center justify-center p-6">
            <div className="flex max-w-sm flex-col items-center gap-4 rounded-xl border border-[var(--border-standard)] bg-[linear-gradient(135deg,rgba(135,175,255,0.08),rgba(255,135,175,0.05))] p-6 text-center shadow-[var(--shadow-lg)] animate-scale-in">
              <div className="relative">
                <div className="absolute inset-0 bg-[radial-gradient(circle,rgba(135,175,255,0.25),transparent_60%)] blur-xl" />
                <MessageCircle className="relative size-10 text-[var(--axon-primary)]" />
              </div>
              <div className="space-y-1.5">
                <h2 className="font-display text-base font-semibold text-[var(--text-primary)]">
                  Start a conversation
                </h2>
                <p className="text-sm leading-relaxed text-[var(--text-secondary)]">
                  Ask Claude to write, analyze, or explore. Paste a URL in the omnibox to run a tool
                  on a webpage.
                </p>
              </div>
              <div className="flex flex-wrap justify-center gap-2 pt-1">
                <span className="inline-flex items-center gap-1 rounded-full border border-[var(--border-subtle)] bg-[rgba(135,175,255,0.08)] px-2.5 py-1 text-xs text-[var(--axon-primary)]">
                  <Send className="size-2.5" />
                  Ask a question
                </span>
                <span className="inline-flex items-center gap-1 rounded-full border border-[var(--border-subtle)] bg-[rgba(255,135,175,0.08)] px-2.5 py-1 text-xs text-[var(--axon-secondary)]">
                  Paste a URL
                </span>
              </div>
            </div>
          </div>
        ) : (
          <>
            {topSpacerHeight > 0 && <div style={{ height: `${topSpacerHeight}px` }} aria-hidden />}
            {virtualMessages.map((msg, index) => {
              const absoluteIndex = virtualStartIndex + index
              const messageKey =
                msg.id ?? `legacy-${absoluteIndex}-${msg.role}-${msg.content.slice(0, 24)}`
              return (
                <MessageBubble
                  key={messageKey}
                  msg={msg}
                  index={absoluteIndex}
                  onRetry={onRetry}
                  copyStatus={copyStatuses.get(messageKey) ?? 'idle'}
                  onCopyError={(content) => {
                    void handleCopyError(content, messageKey)
                  }}
                />
              )
            })}
            {bottomSpacerHeight > 0 && (
              <div style={{ height: `${bottomSpacerHeight}px` }} aria-hidden />
            )}
          </>
        )}

        {isLoading && (
          <div className="flex items-start gap-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-elevated)] px-3 py-2.5 shadow-[var(--shadow-sm)] animate-fade-in">
            <div className="mt-0.5 flex shrink-0 gap-0.5">
              {[0, 1, 2].map((i) => (
                <span
                  key={i}
                  className="inline-block size-1.5 rounded-full bg-[var(--axon-primary)]"
                  style={{ animation: `breathing 1.4s ease-in-out ${i * 200}ms infinite` }}
                />
              ))}
            </div>
            <div className="min-w-0 flex-1">
              <span className="animate-breathing text-sm text-[var(--text-secondary)]">
                {formatStreamPhaseLabel(streamingPhase)}…
              </span>
            </div>
            {onCancelRequest && (
              <button
                type="button"
                onClick={onCancelRequest}
                className="ml-auto inline-flex items-center gap-1 rounded border border-[rgba(255,135,135,0.3)] bg-[rgba(127,29,29,0.28)] px-1.5 py-0.5 text-[length:var(--text-2xs)] text-rose-200"
              >
                <Square className="size-2.5" />
                Stop
              </button>
            )}
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
