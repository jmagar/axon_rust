'use client'

import type React from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTimedNotice } from '@/hooks/use-timed-notice'
import type { ChatStreamEvent } from '@/lib/pulse/chat-api'
import { runChatPrompt, runSourcePrompt } from '@/lib/pulse/chat-api'
import { parseClaudeAssistantPayload } from '@/lib/pulse/claude-response'
import type { ValidationResult } from '@/lib/pulse/doc-ops'
import { validateDocOperations } from '@/lib/pulse/doc-ops'
import { checkPermission } from '@/lib/pulse/permissions'
import { detectPulsePromptIntent } from '@/lib/pulse/prompt-intent'
import type {
  AcpConfigOption,
  DocOperation,
  PulseAgent,
  PulseMessageBlock,
  PulseModel,
  PulsePermissionLevel,
  PulseToolUse,
} from '@/lib/pulse/types'
import type { ChatMessage } from '@/lib/pulse/workspace-persistence'
import type { WsServerMsg } from '@/lib/ws-protocol'

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

/** Snapshot of config values read from configRef at prompt submission time. */
interface PromptConfig {
  chatSessionId: string | null
  documentMarkdown: string
  activeThreadSources: string[]
  scrapedContext: { url: string; markdown: string } | null
  permissionLevel: PulsePermissionLevel
  agent: PulseAgent
  model: PulseModel
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

/** Mutable accumulator shared across streaming event handlers for a single prompt. */
interface StreamAccumulator {
  partialText: string
  draftAdded: boolean
  assistantDraftId: string | undefined
  partialTools: PulseToolUse[]
  partialBlocks: PulseMessageBlock[]
}

// ── Extracted helpers ───────────────────────────────────────────────────────

/**
 * Handle the "source" intent — index URLs and produce a summary message.
 * Returns true if handled (caller should return early).
 */
async function handleSourceIntent(
  urls: string[],
  signal: AbortSignal,
  promptId: number,
  inFlightRef: React.RefObject<number>,
  setIndexedSources: React.Dispatch<React.SetStateAction<string[]>>,
  setActiveThreadSources: React.Dispatch<React.SetStateAction<string[]>>,
  setScrapedContext: React.Dispatch<React.SetStateAction<{ url: string; markdown: string } | null>>,
  setChatHistoryTracked: (action: React.SetStateAction<ChatMessage[]>) => void,
  createMessage: (partial: Omit<ChatMessage, 'id' | 'createdAt'>) => ChatMessage,
): Promise<void> {
  const result = await runSourcePrompt(urls, signal)
  if (inFlightRef.current !== promptId) return
  setIndexedSources((prev) => {
    const next = [...prev]
    for (const url of result.indexed) {
      if (!next.includes(url)) next.push(url)
    }
    return next.slice(-200)
  })
  setActiveThreadSources((prev) => {
    const merged = [...prev]
    for (const url of result.indexed) {
      if (!merged.includes(url)) merged.push(url)
    }
    return merged.slice(-200)
  })
  if (result.markdownBySrc && Object.keys(result.markdownBySrc).length > 0) {
    const firstUrl = result.indexed[0]
    const md = firstUrl ? result.markdownBySrc[firstUrl] : undefined
    if (md && md.length > 0) {
      setScrapedContext({ url: firstUrl ?? '', markdown: md })
    }
  }
  const sourceList = result.indexed.map((url) => `- ${url}`).join('\n')
  const assistantMessage = [
    'Indexed new sources into Axon.',
    '',
    sourceList,
    '',
    'Ask a follow-up question to use this fresh context.',
  ].join('\n')
  setChatHistoryTracked((prev) => [
    ...prev,
    createMessage({ role: 'assistant', content: assistantMessage }),
  ])
}

/**
 * Build the onEvent callback for streaming chat responses.
 * Each event type is handled in a focused branch, updating the accumulator
 * and calling the appropriate state setters.
 */
function makeStreamEventHandler(
  acc: StreamAccumulator,
  promptId: number,
  inFlightRef: React.RefObject<number>,
  assistantDraft: ChatMessage,
  setStreamPhase: React.Dispatch<
    React.SetStateAction<'started' | 'thinking' | 'finalizing' | null>
  >,
  setLiveToolUses: React.Dispatch<React.SetStateAction<PulseToolUse[]>>,
  setChatHistoryTracked: (action: React.SetStateAction<ChatMessage[]>) => void,
  updateChatMessage: (messageId: string, transform: (m: ChatMessage) => ChatMessage) => void,
  onAcpConfigUpdate?: (options: AcpConfigOption[]) => void,
): { handler: (event: ChatStreamEvent) => void; flush: () => void } {
  let deltaFlushTimer: ReturnType<typeof setTimeout> | null = null
  let pendingDeltaFlush = false

  function ensureDraftAdded() {
    if (!acc.draftAdded) {
      acc.draftAdded = true
      setChatHistoryTracked((prev) => [...prev, assistantDraft])
    }
  }

  function flushDeltaToReact() {
    deltaFlushTimer = null
    pendingDeltaFlush = false
    updateChatMessage(assistantDraft.id!, (m) => ({
      ...m,
      content: acc.partialText,
      blocks: [...acc.partialBlocks],
    }))
  }

  function scheduleDeltaFlush() {
    if (pendingDeltaFlush) return
    pendingDeltaFlush = true
    deltaFlushTimer = setTimeout(flushDeltaToReact, 50)
  }

  function flush() {
    if (deltaFlushTimer !== null) {
      clearTimeout(deltaFlushTimer)
      flushDeltaToReact()
    }
  }

  const handler = (event: ChatStreamEvent) => {
    if (inFlightRef.current !== promptId) return

    if (event.type === 'status' && event.phase) {
      setStreamPhase(event.phase)
      return
    }

    if (event.type === 'thinking_content' && event.content) {
      ensureDraftAdded()
      const lastBlock = acc.partialBlocks[acc.partialBlocks.length - 1]
      if (lastBlock?.type === 'thinking') {
        acc.partialBlocks[acc.partialBlocks.length - 1] = {
          type: 'thinking',
          content: event.content,
        }
      } else {
        acc.partialBlocks.push({ type: 'thinking', content: event.content })
      }
      updateChatMessage(assistantDraft.id!, (m) => ({ ...m, blocks: [...acc.partialBlocks] }))
      return
    }

    if (event.type === 'assistant_delta' && event.delta) {
      ensureDraftAdded()
      acc.partialText += event.delta
      const lastBlock = acc.partialBlocks[acc.partialBlocks.length - 1]
      if (lastBlock?.type === 'text') {
        acc.partialBlocks[acc.partialBlocks.length - 1] = {
          ...lastBlock,
          content: lastBlock.content + event.delta,
        }
      } else {
        acc.partialBlocks.push({ type: 'text', content: event.delta })
      }
      // Throttle React updates: accumulate deltas, flush at most every 50ms
      scheduleDeltaFlush()
      return
    }

    if (event.type === 'tool_use' && event.tool) {
      // Flush any pending text delta before adding tool use
      flush()
      ensureDraftAdded()
      acc.partialTools.push(event.tool)
      acc.partialBlocks.push({
        type: 'tool_use',
        name: event.tool.name,
        input: event.tool.input,
      })
      setLiveToolUses([...acc.partialTools])
      updateChatMessage(assistantDraft.id!, (m) => ({
        ...m,
        toolUses: [...acc.partialTools],
        blocks: [...acc.partialBlocks],
      }))
      return
    }

    if (event.type === 'config_options_update' && event.configOptions) {
      onAcpConfigUpdate?.(event.configOptions)
    }
  }

  return { handler, flush }
}

/**
 * Process the final chat response — update metadata, finalize the draft
 * message, and handle doc operations / permissions.
 */
function finalizeStreamResponse(
  data: Awaited<ReturnType<typeof runChatPrompt>>,
  acc: StreamAccumulator,
  cfg: PromptConfig,
  setChatSessionId: React.Dispatch<React.SetStateAction<string | null>>,
  setLastResponseLatencyMs: React.Dispatch<React.SetStateAction<number | null>>,
  setLastResponseModel: React.Dispatch<React.SetStateAction<PulseModel | null>>,
  setLastContextStats: React.Dispatch<
    React.SetStateAction<{ contextCharsTotal: number; contextBudgetChars: number } | null>
  >,
  updateChatMessage: (messageId: string, transform: (m: ChatMessage) => ChatMessage) => void,
  onApplyOperations: (ops: DocOperation[]) => void,
  onPendingOps: (ops: DocOperation[] | null) => void,
  onPendingValidation: (v: ValidationResult | null) => void,
): void {
  if (data.sessionId) setChatSessionId(data.sessionId)
  if (data.metadata) {
    setLastResponseLatencyMs(data.metadata.elapsedMs)
    setLastResponseModel(data.metadata.model)
    setLastContextStats({
      contextCharsTotal: data.metadata.contextCharsTotal,
      contextBudgetChars: data.metadata.contextBudgetChars,
    })
  }

  updateChatMessage(acc.assistantDraftId!, (m) => ({
    ...m,
    content: data.text,
    citations: data.citations,
    operations: data.operations,
    toolUses: data.toolUses,
    blocks: data.blocks,
  }))

  if (data.operations.length > 0) {
    const permission = checkPermission(cfg.permissionLevel, data.operations, {
      isCurrentDoc: true,
      currentDocMarkdown: cfg.documentMarkdown,
    })
    if (permission.allowed && !permission.requiresConfirmation) {
      onApplyOperations(data.operations)
    } else if (permission.allowed && permission.requiresConfirmation) {
      const validation = validateDocOperations(data.operations, cfg.documentMarkdown)
      onPendingOps(data.operations)
      onPendingValidation(validation)
    }
  }
}

/**
 * Handle errors during prompt execution — recover partial content when
 * possible, otherwise display the error in-place.
 */
function handlePromptError(
  err: unknown,
  acc: StreamAccumulator,
  trimmed: string,
  showRequestNotice: (msg: string) => void,
  updateChatMessage: (messageId: string, transform: (m: ChatMessage) => ChatMessage) => void,
  setChatHistoryTracked: (action: React.SetStateAction<ChatMessage[]>) => void,
  createMessage: (partial: Omit<ChatMessage, 'id' | 'createdAt'>) => ChatMessage,
): void {
  if (err instanceof Error && err.name === 'AbortError') {
    showRequestNotice('Request stopped. Partial response preserved.')
    return
  }
  const parsedPartial =
    acc.draftAdded && acc.assistantDraftId ? parseClaudeAssistantPayload(acc.partialText) : null
  const message = err instanceof Error ? err.message : 'Unknown error'
  if (parsedPartial?.text && acc.assistantDraftId) {
    updateChatMessage(acc.assistantDraftId, (m) => ({ ...m, content: parsedPartial.text }))
  } else if (acc.draftAdded && acc.assistantDraftId) {
    updateChatMessage(acc.assistantDraftId, (m) => ({
      ...m,
      content: message,
      isError: true,
      retryPrompt: trimmed,
    }))
  } else {
    setChatHistoryTracked((prev) => [
      ...prev,
      createMessage({
        role: 'assistant',
        content: message,
        isError: true,
        retryPrompt: trimmed,
      }),
    ])
  }
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
