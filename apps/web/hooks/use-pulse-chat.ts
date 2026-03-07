'use client'

import type React from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTimedNotice } from '@/hooks/use-timed-notice'
import { runChatPrompt } from '@/lib/pulse/chat-api'
import type { ValidationResult } from '@/lib/pulse/doc-ops'
import { detectPulsePromptIntent } from '@/lib/pulse/prompt-intent'
import type {
  AcpConfigOption,
  AcpPermissionRequest,
  DocOperation,
  PulseAgent,
  PulseModel,
  PulsePermissionLevel,
  PulseToolUse,
} from '@/lib/pulse/types'
import type { ChatMessage } from '@/lib/pulse/workspace-persistence'
import type { WsServerMsg } from '@/lib/ws-protocol'
import type { PromptConfig, StreamAccumulator } from './pulse-chat-helpers'
import {
  finalizeStreamResponse,
  handlePromptError,
  handleSourceIntent,
  makeStreamEventHandler,
} from './pulse-chat-helpers'

// ── Types ───────────────────────────────────────────────────────────────────

interface UsePulseChatInput {
  documentMarkdown: string
  permissionLevel: PulsePermissionLevel
  agent: PulseAgent
  model: PulseModel
  subscribe: (handler: (msg: WsServerMsg) => void) => () => void
  onApplyOperations: (ops: DocOperation[]) => void
  onPendingOps: (ops: DocOperation[] | null) => void
  onPendingValidation: (v: ValidationResult | null) => void
  onAcpConfigUpdate?: (options: AcpConfigOption[]) => void
  onPermissionRequest?: (req: AcpPermissionRequest) => void
  effort?: string
  maxTurns?: number
  maxBudgetUsd?: number
  appendSystemPrompt?: string
  disableSlashCommands?: boolean
  noSessionPersistence?: boolean
  fallbackModel?: string
  allowedTools?: string
  disallowedTools?: string
}

// ── Main hook ───────────────────────────────────────────────────────────────

export function usePulseChat({
  documentMarkdown,
  permissionLevel,
  agent,
  model,
  subscribe,
  onApplyOperations,
  onPendingOps,
  onPendingValidation,
  onAcpConfigUpdate,
  onPermissionRequest,
  effort,
  maxTurns,
  maxBudgetUsd,
  appendSystemPrompt,
  disableSlashCommands,
  noSessionPersistence,
  fallbackModel,
  allowedTools,
  disallowedTools,
}: UsePulseChatInput) {
  const [chatHistory, setChatHistory] = useState<ChatMessage[]>([])
  const [isChatLoading, setIsChatLoading] = useState(false)
  const [chatSessionId, setChatSessionId] = useState<string | null>(null)
  const [indexedSources, setIndexedSources] = useState<string[]>([])
  const [activeThreadSources, setActiveThreadSources] = useState<string[]>([])
  const [scrapedContext, setScrapedContext] = useState<{ url: string; markdown: string } | null>(
    null,
  )
  const [lastResponseLatencyMs, setLastResponseLatencyMs] = useState<number | null>(null)
  const [lastResponseModel, setLastResponseModel] = useState<PulseModel | null>(null)
  const [lastContextStats, setLastContextStats] = useState<{
    contextCharsTotal: number
    contextBudgetChars: number
  } | null>(null)
  const [streamPhase, setStreamPhase] = useState<'started' | 'thinking' | 'finalizing' | null>(null)
  const [liveToolUses, setLiveToolUses] = useState<PulseToolUse[]>([])
  const { notice: requestNotice, showNotice: showRequestNotice } = useTimedNotice()

  const chatHistoryRef = useRef<ChatMessage[]>([])
  const inFlightPromptRef = useRef(0)
  const activePromptAbortRef = useRef<AbortController | null>(null)
  const messageIdRef = useRef(0)
  const lastCmdModeRef = useRef('')
  const lastCmdInputRef = useRef('')
  const lastSubmittedPromptRef = useRef<{ text: string; atMs: number } | null>(null)

  // Ref for frequently-changing values used inside handlePrompt — avoids
  // recreating the callback on every keystroke or streaming update (CQ-3).
  const configRef = useRef<PromptConfig>({
    chatSessionId,
    documentMarkdown,
    activeThreadSources,
    scrapedContext,
    permissionLevel,
    agent,
    model,
    effort,
    maxTurns,
    maxBudgetUsd,
    appendSystemPrompt,
    disableSlashCommands,
    noSessionPersistence,
    fallbackModel,
    allowedTools,
    disallowedTools,
  })

  // Keep the ref in sync — single assignment instead of re-creating the object
  // on every render when no values changed. The ref is only read inside
  // handlePrompt, which captures it by reference.
  configRef.current.chatSessionId = chatSessionId
  configRef.current.documentMarkdown = documentMarkdown
  configRef.current.activeThreadSources = activeThreadSources
  configRef.current.scrapedContext = scrapedContext
  configRef.current.permissionLevel = permissionLevel
  configRef.current.agent = agent
  configRef.current.model = model
  configRef.current.effort = effort
  configRef.current.maxTurns = maxTurns
  configRef.current.maxBudgetUsd = maxBudgetUsd
  configRef.current.appendSystemPrompt = appendSystemPrompt
  configRef.current.disableSlashCommands = disableSlashCommands
  configRef.current.noSessionPersistence = noSessionPersistence
  configRef.current.fallbackModel = fallbackModel
  configRef.current.allowedTools = allowedTools
  configRef.current.disallowedTools = disallowedTools

  const setChatHistoryTracked = useCallback((action: React.SetStateAction<ChatMessage[]>) => {
    if (typeof action === 'function') {
      setChatHistory((prev) => {
        const next = action(prev)
        chatHistoryRef.current = next
        return next
      })
    } else {
      chatHistoryRef.current = action
      setChatHistory(action)
    }
  }, [])

  // WS scrape/crawl subscription
  useEffect(() => {
    return subscribe((msg: WsServerMsg) => {
      if (msg.type === 'command.start') {
        lastCmdModeRef.current = msg.data.ctx.mode
        lastCmdInputRef.current = msg.data.ctx.input
      }
      if (msg.type === 'command.output.json' && msg.data.ctx.mode === 'scrape') {
        const data = msg.data.data as Record<string, unknown> | null
        if (data && typeof data.markdown === 'string' && data.markdown.length > 0) {
          setScrapedContext({
            url: typeof data.url === 'string' ? data.url : lastCmdInputRef.current,
            markdown: data.markdown,
          })
        }
      }
      if (
        msg.type === 'command.done' &&
        msg.data.payload.exit_code === 0 &&
        lastCmdModeRef.current === 'crawl'
      ) {
        const url = lastCmdInputRef.current
        if (url) {
          setActiveThreadSources((prev) => (prev.includes(url) ? prev : [...prev, url]))
        }
      }
    })
  }, [subscribe])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (activePromptAbortRef.current) {
        activePromptAbortRef.current.abort()
        activePromptAbortRef.current = null
      }
    }
  }, [])

  const createMessage = useCallback(
    (partial: Omit<ChatMessage, 'id' | 'createdAt'>): ChatMessage => {
      messageIdRef.current += 1
      const now = Date.now()
      return { ...partial, id: `msg-${now}-${messageIdRef.current}`, createdAt: now }
    },
    [],
  )

  const updateChatMessage = useCallback(
    (messageId: string, transform: (message: ChatMessage) => ChatMessage) => {
      setChatHistoryTracked((prev) =>
        prev.map((message) => (message.id === messageId ? transform(message) : message)),
      )
    },
    [setChatHistoryTracked],
  )

  const handlePrompt = useCallback(
    async (prompt: string) => {
      const trimmed = prompt.trim()
      if (!trimmed) return
      const now = Date.now()
      const lastSubmitted = lastSubmittedPromptRef.current
      if (
        activePromptAbortRef.current &&
        lastSubmitted &&
        lastSubmitted.text === trimmed &&
        now - lastSubmitted.atMs < 1500
      ) {
        return
      }
      lastSubmittedPromptRef.current = { text: trimmed, atMs: now }

      const promptId = inFlightPromptRef.current + 1
      inFlightPromptRef.current = promptId
      if (activePromptAbortRef.current) {
        activePromptAbortRef.current.abort()
        showRequestNotice('Previous request replaced by your latest prompt.')
      }
      const controller = new AbortController()
      activePromptAbortRef.current = controller

      setChatHistoryTracked((prev) => {
        const last = prev[prev.length - 1]
        if (
          last &&
          last.role === 'user' &&
          last.content === trimmed &&
          now - (last.createdAt ?? 0) < 1500
        ) {
          return prev
        }
        return [...prev, createMessage({ role: 'user', content: trimmed })]
      })
      setIsChatLoading(true)
      setStreamPhase('started')
      setLiveToolUses([])

      const conversationHistory = chatHistoryRef.current.map((m) => ({
        role: m.role,
        content: m.content.slice(0, 8000),
      }))
      const intent = detectPulsePromptIntent(trimmed)

      const acc: StreamAccumulator = {
        partialText: '',
        draftAdded: false,
        assistantDraftId: undefined,
        partialTools: [],
        partialBlocks: [],
      }

      try {
        if (intent.kind === 'source') {
          await handleSourceIntent(
            intent.urls,
            controller.signal,
            promptId,
            inFlightPromptRef,
            setIndexedSources,
            setActiveThreadSources,
            setScrapedContext,
            setChatHistoryTracked,
            createMessage,
          )
          return
        }

        const boundedPrompt = intent.prompt.slice(0, 8000)
        const assistantDraft = createMessage({
          role: 'assistant',
          content: '',
          toolUses: [],
          blocks: [],
        })
        acc.assistantDraftId = assistantDraft.id

        const cfg = configRef.current
        const { handler: onEvent, flush: flushStream } = makeStreamEventHandler(
          acc,
          promptId,
          inFlightPromptRef,
          assistantDraft,
          setStreamPhase,
          setLiveToolUses,
          setChatHistoryTracked,
          updateChatMessage,
          onAcpConfigUpdate,
          onPermissionRequest,
        )

        const data = await runChatPrompt({
          prompt: boundedPrompt,
          conversationHistory,
          signal: controller.signal,
          onEvent,
          chatSessionId: cfg.chatSessionId,
          documentMarkdown: cfg.documentMarkdown,
          activeThreadSources: cfg.activeThreadSources,
          scrapedContext: cfg.scrapedContext,
          permissionLevel: cfg.permissionLevel,
          agent: cfg.agent,
          model: cfg.model,
          effort: cfg.effort,
          maxTurns: cfg.maxTurns,
          maxBudgetUsd: cfg.maxBudgetUsd,
          appendSystemPrompt: cfg.appendSystemPrompt,
          disableSlashCommands: cfg.disableSlashCommands,
          noSessionPersistence: cfg.noSessionPersistence,
          fallbackModel: cfg.fallbackModel,
          allowedTools: cfg.allowedTools,
          disallowedTools: cfg.disallowedTools,
        })

        if (inFlightPromptRef.current !== promptId) return
        // Flush any throttled deltas before finalizing
        flushStream()
        if (!acc.draftAdded) {
          acc.draftAdded = true
          setChatHistoryTracked((prev) => [...prev, assistantDraft])
        }

        finalizeStreamResponse(
          data,
          acc,
          cfg,
          setChatSessionId,
          setLastResponseLatencyMs,
          setLastResponseModel,
          setLastContextStats,
          updateChatMessage,
          onApplyOperations,
          onPendingOps,
          onPendingValidation,
        )
      } catch (err: unknown) {
        if (inFlightPromptRef.current !== promptId) return
        handlePromptError(
          err,
          acc,
          trimmed,
          showRequestNotice,
          updateChatMessage,
          setChatHistoryTracked,
          createMessage,
        )
      } finally {
        if (activePromptAbortRef.current === controller) activePromptAbortRef.current = null
        if (inFlightPromptRef.current === promptId) {
          setIsChatLoading(false)
          setStreamPhase(null)
          setLiveToolUses([])
        }
      }
    },
    [
      createMessage,
      onAcpConfigUpdate,
      onApplyOperations,
      onPendingOps,
      onPendingValidation,
      onPermissionRequest,
      setChatHistoryTracked,
      showRequestNotice,
      updateChatMessage,
    ],
  )

  const handleCancelPrompt = useCallback(() => {
    if (!activePromptAbortRef.current) return
    activePromptAbortRef.current.abort()
    activePromptAbortRef.current = null
    inFlightPromptRef.current += 1
    setIsChatLoading(false)
    setStreamPhase(null)
    setLiveToolUses([])
    showRequestNotice('Request stopped. Partial response preserved.')
  }, [showRequestNotice])

  return {
    chatHistory,
    setChatHistory: setChatHistoryTracked,
    isChatLoading,
    chatSessionId,
    setChatSessionId,
    indexedSources,
    setIndexedSources,
    activeThreadSources,
    setActiveThreadSources,
    scrapedContext,
    lastResponseLatencyMs,
    setLastResponseLatencyMs,
    lastResponseModel,
    setLastResponseModel,
    lastContextStats,
    streamPhase,
    liveToolUses,
    requestNotice,
    handlePrompt,
    handleCancelPrompt,
    messageIdRef,
  }
}
