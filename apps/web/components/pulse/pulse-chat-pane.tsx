'use client'

import { useVirtualizer } from '@tanstack/react-virtual'
import { MessageCircle, Send, Square, X } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { PulseToolUse } from '@/lib/pulse/types'
import type { ChatMessage } from '@/lib/pulse/workspace-persistence'
import {
  CHAT_SCROLL_STORAGE_KEY,
  formatStreamPhaseLabel,
  MESSAGE_ESTIMATED_HEIGHT,
  MESSAGE_VIRTUAL_THRESHOLD,
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
  resumeSessionId?: string | null
  onClearResumeSession?: () => void
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
  resumeSessionId,
  onClearResumeSession,
}: PulseChatPaneProps) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const sourceListRef = useRef<HTMLDivElement>(null)
  const scrollRafRef = useRef<number | null>(null)
  const scrollStorageTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [isNearBottom, setIsNearBottom] = useState(true)
  const [showJumpToLatest, setShowJumpToLatest] = useState(false)
  const [sourceListOpen, setSourceListOpen] = useState(false)
  const [copyStatuses, setCopyStatuses] = useState<Map<string, 'idle' | 'copied' | 'failed'>>(
    new Map(),
  )
  const [sourceListScrollTop, setSourceListScrollTop] = useState(0)
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
  const shouldVirtualizeMessages = messages.length > MESSAGE_VIRTUAL_THRESHOLD
  const messageVirtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => MESSAGE_ESTIMATED_HEIGHT,
    overscan: 8,
    enabled: shouldVirtualizeMessages,
  })
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

  // Cleanup pending rAF and debounce timers on unmount
  useEffect(() => {
    return () => {
      if (scrollRafRef.current !== null) cancelAnimationFrame(scrollRafRef.current)
      if (scrollStorageTimerRef.current !== null) clearTimeout(scrollStorageTimerRef.current)
    }
  }, [])

  function scrollToBottom(instant?: boolean) {
    const node = scrollRef.current
    if (!node) return
    node.scrollTo({ top: node.scrollHeight, behavior: instant ? 'instant' : 'smooth' })
    setShowJumpToLatest(false)
  }

  useEffect(() => {
    const node = scrollRef.current
    if (!node) return
    try {
      const saved = Number(window.localStorage.getItem(CHAT_SCROLL_STORAGE_KEY) ?? 0)
      if (Number.isFinite(saved) && saved > 0) {
        node.scrollTop = saved
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

  // Track the last message content length to detect streaming updates
  const lastScrolledAtRef = useRef(0)
  useEffect(() => {
    const node = scrollRef.current
    if (!node) return
    if (isNearBottom) {
      // Throttle scroll-to-bottom during streaming: max once per 120ms
      const now = Date.now()
      if (now - lastScrolledAtRef.current < 120) return
      lastScrolledAtRef.current = now
      node.scrollTo({ top: node.scrollHeight, behavior: 'instant' })
      setShowJumpToLatest(false)
    } else if (messages.length > 0) {
      setShowJumpToLatest(true)
    }
  }, [isNearBottom, messages])

  const handleCopyError = useCallback(async (content: string, messageId: string) => {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(content)
      } else {
        const textarea = document.createElement('textarea')
        textarea.value = content
        textarea.style.position = 'fixed'
        textarea.style.opacity = '0'
        document.body.appendChild(textarea)
        textarea.select()
        document.execCommand('copy')
        document.body.removeChild(textarea)
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
  }, [])

  return (
    <div className="flex h-full min-h-0 flex-col">
      {(requestNotice ||
        resumeSessionId ||
        (sourcesExpanded && (activeSources.length > 0 || latestAssistantCitations.length > 0)) ||
        (sourceListOpen && activeSources.length > 0)) && (
        <div className="border-b border-[var(--border-subtle)] bg-[linear-gradient(120deg,rgba(175,215,255,0.05),rgba(255,135,175,0.03))] px-3 py-2">
          {resumeSessionId && (
            <div className="mt-1 flex items-center justify-between gap-2 rounded border border-[rgba(175,215,255,0.3)] bg-[rgba(175,215,255,0.08)] px-1.5 py-1 ui-meta text-[var(--text-secondary)]">
              <span className="truncate">
                Resumed session: <code>{resumeSessionId}</code>
              </span>
              {onClearResumeSession && (
                <button
                  type="button"
                  onClick={onClearResumeSession}
                  className="shrink-0 rounded border border-[var(--border-subtle)] px-1.5 py-0.5 text-[10px] text-[var(--text-dim)] hover:text-[var(--text-primary)]"
                >
                  Clear resume
                </button>
              )}
            </div>
          )}
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
                      className="inline-flex max-w-[160px] items-center gap-1 truncate rounded-full border border-[rgba(95,135,175,0.28)] bg-[rgba(15,23,42,0.48)] px-1.5 py-0.5 text-[length:var(--text-2xs)] text-[var(--text-dim)]"
                      title={source}
                    >
                      {(() => {
                        try {
                          return new URL(source).hostname
                        } catch {
                          return source
                        }
                      })()}
                      <button
                        type="button"
                        onClick={() => onRemoveSource(source)}
                        aria-label={`Remove ${source} from thread context`}
                        className="inline-flex items-center justify-center rounded-full text-[var(--text-dim)] transition-colors hover:text-[var(--text-primary)]"
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
                      className="ui-chip inline-flex items-center rounded-full border border-[rgba(255,135,175,0.2)] bg-[rgba(10,18,35,0.42)] px-1.5 py-0.5 text-[var(--text-dim)]"
                      aria-expanded={sourceListOpen}
                    >
                      {sourceListOpen ? 'Hide sources' : `+${hiddenSourceCount} more`}
                    </button>
                  )}
                  {indexedSources.length > activeSources.length && (
                    <span className="ui-chip inline-flex items-center rounded-full border border-[rgba(255,135,175,0.2)] bg-[rgba(10,18,35,0.42)] px-1.5 py-0.5 text-[var(--text-dim)]">
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
                        className="block rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.45)] px-1.5 py-1 transition-colors hover:border-[var(--focus-ring-color)]"
                      >
                        <div className="mb-0.5 flex items-center justify-between gap-1">
                          <p className="line-clamp-1 text-[length:var(--text-xs)] font-semibold text-[var(--axon-secondary)]">
                            {citation.title}
                          </p>
                          <span className="ui-chip rounded border border-[rgba(175,215,255,0.24)] bg-[rgba(175,215,255,0.1)] px-1 py-0.5 text-[var(--text-dim)]">
                            {citation.collection}
                          </span>
                        </div>
                        <p className="line-clamp-2 text-[length:var(--text-xs)] leading-[var(--leading-tight)] text-[var(--text-dim)]">
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
                    <span className="truncate text-[length:var(--text-2xs)] text-[var(--text-dim)]">
                      {source}
                    </span>
                    <button
                      type="button"
                      onClick={() => onRemoveSource(source)}
                      className="inline-flex items-center rounded border border-[rgba(95,135,175,0.25)] px-1 py-0.5 text-[length:var(--text-2xs)] text-[var(--text-dim)]"
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
          if (scrollRafRef.current !== null) return
          scrollRafRef.current = requestAnimationFrame(() => {
            scrollRafRef.current = null
            const node = scrollRef.current
            if (!node) return
            const nearBottom = node.scrollHeight - (node.scrollTop + node.clientHeight) < 42
            setIsNearBottom(nearBottom)
            if (nearBottom) setShowJumpToLatest(false)
            // Debounce localStorage writes to avoid synchronous I/O on every scroll frame
            if (scrollStorageTimerRef.current !== null) {
              clearTimeout(scrollStorageTimerRef.current)
            }
            scrollStorageTimerRef.current = setTimeout(() => {
              try {
                window.localStorage.setItem(CHAT_SCROLL_STORAGE_KEY, String(node.scrollTop))
              } catch {
                // Ignore storage failures.
              }
            }, 150)
          })
        }}
        className="flex min-h-0 flex-1 flex-col space-y-2.5 overflow-y-auto overscroll-y-contain px-3 py-2.5"
      >
        {messages.length === 0 ? (
          <div className="flex h-full items-center justify-center p-6">
            <div className="flex max-w-sm flex-col items-center gap-4 rounded-xl border border-[var(--border-standard)] bg-[linear-gradient(135deg,rgba(135,175,255,0.08),rgba(255,135,175,0.05))] p-6 text-center shadow-[var(--shadow-lg)] animate-scale-in">
              <div className="relative">
                <div className="absolute inset-0 bg-[radial-gradient(circle,rgba(135,175,255,0.25),transparent_60%)] blur-xl" />
                <MessageCircle className="relative size-10 text-[var(--axon-primary)]" />
              </div>
              <div className="space-y-1.5">
                {resumeSessionId ? (
                  <>
                    <h2 className="font-display text-base font-semibold text-[var(--text-primary)]">
                      Session Resumed
                    </h2>
                    <p className="text-sm leading-relaxed text-[var(--text-secondary)]">
                      Continuing session <code>{resumeSessionId}</code>. Send a message in the
                      omnibox to continue this thread.
                    </p>
                  </>
                ) : (
                  <>
                    <h2 className="font-display text-base font-semibold text-[var(--text-primary)]">
                      Start a conversation
                    </h2>
                    <p className="text-sm leading-relaxed text-[var(--text-secondary)]">
                      Ask Claude to write, analyze, or explore. Paste a URL in the omnibox to run a
                      tool on a webpage.
                    </p>
                  </>
                )}
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
        ) : shouldVirtualizeMessages ? (
          <div
            style={{
              height: `${messageVirtualizer.getTotalSize()}px`,
              position: 'relative',
            }}
          >
            {messageVirtualizer.getVirtualItems().map((virtualRow) => {
              const msg = messages[virtualRow.index]
              const messageKey =
                msg.id ?? `legacy-${virtualRow.index}-${msg.role}-${msg.content.slice(0, 24)}`
              return (
                <div
                  key={messageKey}
                  data-index={virtualRow.index}
                  ref={messageVirtualizer.measureElement}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <MessageBubble
                    msg={msg}
                    index={virtualRow.index}
                    onRetry={onRetry}
                    copyStatus={copyStatuses.get(messageKey) ?? 'idle'}
                    onCopyError={handleCopyError}
                  />
                </div>
              )
            })}
          </div>
        ) : (
          messages.map((msg, index) => {
            const messageKey = msg.id ?? `legacy-${index}-${msg.role}-${msg.content.slice(0, 24)}`
            return (
              <MessageBubble
                key={messageKey}
                msg={msg}
                index={index}
                onRetry={onRetry}
                copyStatus={copyStatuses.get(messageKey) ?? 'idle'}
                onCopyError={handleCopyError}
              />
            )
          })
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
                className="ml-auto inline-flex items-center gap-1 rounded border border-[var(--border-subtle)] bg-[var(--surface-elevated)] px-1.5 py-0.5 text-[length:var(--text-2xs)] text-[var(--text-dim)] hover:border-[var(--border-accent)] hover:text-[var(--text-secondary)]"
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
            onClick={() => scrollToBottom()}
            className="ui-chip sticky bottom-2 ml-auto inline-flex items-center rounded-full border border-[rgba(175,215,255,0.28)] bg-[rgba(10,18,35,0.72)] px-2 py-1 text-[var(--text-dim)] shadow-[0_4px_12px_rgba(3,7,18,0.32)]"
          >
            Jump to latest
          </button>
        )}
      </div>
    </div>
  )
}
