'use client'

import { useCallback, useEffect, useRef, useState } from 'react'
import type { ChatStreamEvent } from '@/lib/pulse/chat-api'
import { runChatPrompt, runSourcePrompt } from '@/lib/pulse/chat-api'
import type { ValidationResult } from '@/lib/pulse/doc-ops'
import { validateDocOperations } from '@/lib/pulse/doc-ops'
import { checkPermission } from '@/lib/pulse/permissions'
import { detectPulsePromptIntent } from '@/lib/pulse/prompt-intent'
import type {
  DocOperation,
  PulseMessageBlock,
  PulseModel,
  PulsePermissionLevel,
  PulseToolUse,
} from '@/lib/pulse/types'
import type { ChatMessage } from '@/lib/pulse/workspace-persistence'
import type { WsServerMsg } from '@/lib/ws-protocol'

interface UsePulseChatInput {
  documentMarkdown: string
  permissionLevel: PulsePermissionLevel
  model: PulseModel
  subscribe: (handler: (msg: WsServerMsg) => void) => () => void
  onApplyOperations: (ops: DocOperation[]) => void
  onPendingOps: (ops: DocOperation[] | null) => void
  onPendingValidation: (v: ValidationResult | null) => void
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

export function usePulseChat({
  documentMarkdown,
  permissionLevel,
  model,
  subscribe,
  onApplyOperations,
  onPendingOps,
  onPendingValidation,
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
  const [requestNotice, setRequestNotice] = useState<string | null>(null)

  const chatHistoryRef = useRef<ChatMessage[]>([])
  const inFlightPromptRef = useRef(0)
  const activePromptAbortRef = useRef<AbortController | null>(null)
  const messageIdRef = useRef(0)
  const lastCmdModeRef = useRef('')
  const lastCmdInputRef = useRef('')

  // Keep chatHistoryRef in sync
  useEffect(() => {
    chatHistoryRef.current = chatHistory
  }, [chatHistory])

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
      setChatHistory((prev) =>
        prev.map((message) => (message.id === messageId ? transform(message) : message)),
      )
    },
    [],
  )

  const handlePrompt = useCallback(
    async (prompt: string) => {
      const trimmed = prompt.trim()
      if (!trimmed) return

      const promptId = inFlightPromptRef.current + 1
      inFlightPromptRef.current = promptId
      if (activePromptAbortRef.current) {
        activePromptAbortRef.current.abort()
        setRequestNotice('Previous request replaced by your latest prompt.')
        window.setTimeout(() => setRequestNotice(null), 1800)
      }
      const controller = new AbortController()
      activePromptAbortRef.current = controller

      setChatHistory((prev) => [...prev, createMessage({ role: 'user', content: trimmed })])
      setIsChatLoading(true)
      setStreamPhase('started')
      setLiveToolUses([])

      const conversationHistory = chatHistoryRef.current.map((m) => ({
        role: m.role,
        content: m.content.slice(0, 8000),
      }))
      const intent = detectPulsePromptIntent(trimmed)

      try {
        if (intent.kind === 'source') {
          const result = await runSourcePrompt(intent.urls, controller.signal)
          if (inFlightPromptRef.current !== promptId) return
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
          setChatHistory((prev) => [
            ...prev,
            createMessage({ role: 'assistant', content: assistantMessage }),
          ])
          return
        }

        const boundedPrompt = intent.prompt.slice(0, 8000)
        const assistantDraft = createMessage({
          role: 'assistant',
          content: '',
          toolUses: [],
          blocks: [],
        })

        let partialText = ''
        const partialTools: PulseToolUse[] = []
        const partialBlocks: PulseMessageBlock[] = []
        let draftAdded = false
        function ensureDraftAdded() {
          if (!draftAdded) {
            draftAdded = true
            setChatHistory((prev) => [...prev, assistantDraft])
          }
        }

        const data = await runChatPrompt({
          prompt: boundedPrompt,
          conversationHistory,
          signal: controller.signal,
          onEvent: (event: ChatStreamEvent) => {
            if (inFlightPromptRef.current !== promptId) return
            if (event.type === 'status' && event.phase) {
              setStreamPhase(event.phase)
              return
            }
            if (event.type === 'thinking_content' && event.content) {
              ensureDraftAdded()
              // Partial-message update: update the last thinking block in-place to avoid
              // stacking multiple "Reasoning" boxes as the thinking grows incrementally.
              const lastBlock = partialBlocks[partialBlocks.length - 1]
              if (lastBlock?.type === 'thinking') {
                partialBlocks[partialBlocks.length - 1] = {
                  type: 'thinking',
                  content: event.content,
                }
              } else {
                partialBlocks.push({ type: 'thinking', content: event.content })
              }
              updateChatMessage(assistantDraft.id!, (m) => ({ ...m, blocks: [...partialBlocks] }))
              return
            }
            if (event.type === 'assistant_delta' && event.delta) {
              ensureDraftAdded()
              partialText += event.delta
              const lastBlock = partialBlocks[partialBlocks.length - 1]
              if (lastBlock?.type === 'text') {
                partialBlocks[partialBlocks.length - 1] = {
                  ...lastBlock,
                  content: lastBlock.content + event.delta,
                }
              } else {
                partialBlocks.push({ type: 'text', content: event.delta })
              }
              updateChatMessage(assistantDraft.id!, (m) => ({
                ...m,
                content: partialText,
                blocks: [...partialBlocks],
              }))
              return
            }
            if (event.type === 'tool_use' && event.tool) {
              ensureDraftAdded()
              partialTools.push(event.tool)
              partialBlocks.push({
                type: 'tool_use',
                name: event.tool.name,
                input: event.tool.input,
              })
              setLiveToolUses([...partialTools])
              updateChatMessage(assistantDraft.id!, (m) => ({
                ...m,
                toolUses: [...partialTools],
                blocks: [...partialBlocks],
              }))
            }
          },
          chatSessionId,
          documentMarkdown,
          activeThreadSources,
          scrapedContext,
          permissionLevel,
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

        if (inFlightPromptRef.current !== promptId) return
        ensureDraftAdded()
        if (data.sessionId) setChatSessionId(data.sessionId)
        if (data.metadata) {
          setLastResponseLatencyMs(data.metadata.elapsedMs)
          setLastResponseModel(data.metadata.model)
          setLastContextStats({
            contextCharsTotal: data.metadata.contextCharsTotal,
            contextBudgetChars: data.metadata.contextBudgetChars,
          })
        }

        updateChatMessage(assistantDraft.id!, (m) => ({
          ...m,
          content: data.text || partialText,
          citations: data.citations,
          operations: data.operations,
          toolUses: data.toolUses,
          blocks: data.blocks,
        }))

        if (data.operations.length > 0) {
          const permission = checkPermission(permissionLevel, data.operations, {
            isCurrentDoc: true,
            currentDocMarkdown: documentMarkdown,
          })
          if (permission.allowed && !permission.requiresConfirmation) {
            onApplyOperations(data.operations)
          } else if (permission.allowed && permission.requiresConfirmation) {
            const validation = validateDocOperations(data.operations, documentMarkdown)
            onPendingOps(data.operations)
            onPendingValidation(validation)
          }
        }
      } catch (err: unknown) {
        if (err instanceof Error && err.name === 'AbortError') {
          setRequestNotice('Request stopped. Partial response preserved.')
          window.setTimeout(() => setRequestNotice(null), 1800)
          return
        }
        if (inFlightPromptRef.current !== promptId) return
        const message = err instanceof Error ? err.message : 'Unknown error'
        setChatHistory((prev) => [
          ...prev,
          createMessage({
            role: 'assistant',
            content: message,
            isError: true,
            retryPrompt: trimmed,
          }),
        ])
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
      activeThreadSources,
      allowedTools,
      appendSystemPrompt,
      chatSessionId,
      createMessage,
      disableSlashCommands,
      disallowedTools,
      documentMarkdown,
      effort,
      fallbackModel,
      maxBudgetUsd,
      maxTurns,
      model,
      noSessionPersistence,
      onApplyOperations,
      onPendingOps,
      onPendingValidation,
      permissionLevel,
      scrapedContext,
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
    setRequestNotice('Request stopped. Partial response preserved.')
    window.setTimeout(() => setRequestNotice(null), 1800)
  }, [])

  return {
    chatHistory,
    setChatHistory,
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
