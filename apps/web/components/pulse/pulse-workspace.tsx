'use client'

import { useCallback, useEffect, useRef, useState } from 'react'
import { CrawlFileExplorer } from '@/components/crawl-file-explorer'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { useWsMessages } from '@/hooks/use-ws-messages'
import type { ValidationResult } from '@/lib/pulse/doc-ops'
import { validateDocOperations } from '@/lib/pulse/doc-ops'
import { checkPermission } from '@/lib/pulse/permissions'
import { detectPulsePromptIntent } from '@/lib/pulse/prompt-intent'
import type {
  DocOperation,
  PulseChatResponse,
  PulseMessageBlock,
  PulseModel,
  PulsePermissionLevel,
  PulseSourceResponse,
  PulseToolUse,
} from '@/lib/pulse/types'
import type { WsServerMsg } from '@/lib/ws-protocol'
import { PulseChatPane } from './pulse-chat-pane'
import { PulseEditorPane } from './pulse-editor-pane'
import { PulseOpConfirmation } from './pulse-op-confirmation'
import { PulseToolbar } from './pulse-toolbar'

export interface ChatMessage {
  id?: string
  role: 'user' | 'assistant'
  content: string
  createdAt?: number
  citations?: PulseChatResponse['citations']
  operations?: PulseChatResponse['operations']
  toolUses?: PulseToolUse[]
  blocks?: PulseMessageBlock[]
  isError?: boolean
  retryPrompt?: string
}

const PULSE_WORKSPACE_STATE_KEY = 'axon.web.pulse.workspace-state.v2'

type DesktopViewMode = 'chat' | 'editor' | 'both'
type DesktopPaneOrder = 'editor-first' | 'chat-first'

type PersistedPulseWorkspaceState = {
  permissionLevel: PulsePermissionLevel
  model: PulseModel
  documentMarkdown: string
  chatHistory: ChatMessage[]
  documentTitle: string
  chatSessionId: string | null
  indexedSources: string[]
  activeThreadSources: string[]
  desktopSplitPercent: number
  mobileSplitPercent: number
  lastResponseLatencyMs: number | null
  lastResponseModel: PulseModel | null
  desktopViewMode: DesktopViewMode
  desktopPaneOrder: DesktopPaneOrder
  savedAt: number
}

function clampSplit(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value))
}

function parsePersistedWorkspaceState(raw: string | null): PersistedPulseWorkspaceState | null {
  if (!raw) return null
  try {
    const parsed = JSON.parse(raw) as Partial<PersistedPulseWorkspaceState>
    if (!parsed || typeof parsed !== 'object') return null
    if (typeof parsed.documentTitle !== 'string' || typeof parsed.documentMarkdown !== 'string') {
      return null
    }
    const model: PulseModel =
      parsed.model === 'opus' || parsed.model === 'haiku' || parsed.model === 'sonnet'
        ? parsed.model
        : 'sonnet'
    const permissionLevel: PulsePermissionLevel =
      parsed.permissionLevel === 'plan' ||
      parsed.permissionLevel === 'accept-edits' ||
      parsed.permissionLevel === 'bypass-permissions'
        ? parsed.permissionLevel
        : 'accept-edits'
    return {
      permissionLevel,
      model,
      documentMarkdown: parsed.documentMarkdown,
      chatHistory: Array.isArray(parsed.chatHistory) ? parsed.chatHistory.slice(-250) : [],
      documentTitle: parsed.documentTitle,
      chatSessionId: typeof parsed.chatSessionId === 'string' ? parsed.chatSessionId : null,
      indexedSources: Array.isArray(parsed.indexedSources) ? parsed.indexedSources.slice(-50) : [],
      activeThreadSources: Array.isArray(parsed.activeThreadSources)
        ? parsed.activeThreadSources.slice(-50)
        : [],
      desktopSplitPercent: clampSplit(Number(parsed.desktopSplitPercent ?? 62), 42, 74),
      mobileSplitPercent: clampSplit(Number(parsed.mobileSplitPercent ?? 56), 35, 70),
      lastResponseLatencyMs:
        typeof parsed.lastResponseLatencyMs === 'number' ? parsed.lastResponseLatencyMs : null,
      lastResponseModel:
        parsed.lastResponseModel === 'sonnet' ||
        parsed.lastResponseModel === 'opus' ||
        parsed.lastResponseModel === 'haiku'
          ? parsed.lastResponseModel
          : null,
      desktopViewMode:
        parsed.desktopViewMode === 'chat' ||
        parsed.desktopViewMode === 'editor' ||
        parsed.desktopViewMode === 'both'
          ? parsed.desktopViewMode
          : 'both',
      desktopPaneOrder: parsed.desktopPaneOrder === 'chat-first' ? 'chat-first' : 'editor-first',
      savedAt: typeof parsed.savedAt === 'number' ? parsed.savedAt : Date.now(),
    }
  } catch {
    return null
  }
}

export function PulseWorkspace() {
  const {
    workspacePrompt,
    workspacePromptVersion,
    updateWorkspaceContext,
    pulseModel,
    pulsePermissionLevel,
    setPulseModel,
    setPulsePermissionLevel,
    crawlFiles,
    selectedFile,
    selectFile,
    currentJobId,
    markdownContent,
  } = useWsMessages()
  const { subscribe } = useAxonWs()
  const [documentMarkdown, setDocumentMarkdown] = useState('')
  const [chatHistory, setChatHistory] = useState<ChatMessage[]>([])
  const [isChatLoading, setIsChatLoading] = useState(false)
  const [documentTitle, setDocumentTitle] = useState('Untitled')
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle')
  const [pendingOps, setPendingOps] = useState<DocOperation[] | null>(null)
  const [pendingValidation, setPendingValidation] = useState<ValidationResult | null>(null)
  const [chatSessionId, setChatSessionId] = useState<string | null>(null)
  const [indexedSources, setIndexedSources] = useState<string[]>([])
  const [activeThreadSources, setActiveThreadSources] = useState<string[]>([])
  const [scrapedContext, setScrapedContext] = useState<{ url: string; markdown: string } | null>(
    null,
  )
  // Track the last command's mode+input so we can record crawled URLs on done.
  const lastCmdModeRef = useRef('')
  const lastCmdInputRef = useRef('')
  const [desktopSplitPercent, setDesktopSplitPercent] = useState(62)
  const [mobileSplitPercent, setMobileSplitPercent] = useState(56)
  const [isDesktop, setIsDesktop] = useState(false)
  const [mobilePane, setMobilePane] = useState<'chat' | 'editor'>('chat')
  const [desktopViewMode, setDesktopViewMode] = useState<DesktopViewMode>('both')
  const [desktopPaneOrder, setDesktopPaneOrder] = useState<DesktopPaneOrder>('editor-first')
  const [lastResponseLatencyMs, setLastResponseLatencyMs] = useState<number | null>(null)
  const [lastResponseModel, setLastResponseModel] = useState<PulseModel | null>(null)
  const [requestNotice, setRequestNotice] = useState<string | null>(null)
  const model = pulseModel as PulseModel
  const permissionLevel = pulsePermissionLevel as PulsePermissionLevel
  const [lastContextStats, setLastContextStats] = useState<{
    promptChars: number
    documentChars: number
    conversationChars: number
    citationChars: number
    contextCharsTotal: number
    contextBudgetChars: number
  } | null>(null)
  const autosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const autosaveAbortRef = useRef<AbortController | null>(null)
  const lastSavedSnapshotRef = useRef('')
  const lastHandledPromptVersionRef = useRef(0)
  const chatHistoryRef = useRef<ChatMessage[]>([])
  const inFlightPromptRef = useRef(0)
  const activePromptAbortRef = useRef<AbortController | null>(null)
  const messageIdRef = useRef(0)
  const desktopSplitPercentRef = useRef(desktopSplitPercent)
  const mobileSplitPercentRef = useRef(mobileSplitPercent)
  const hasHydratedPersistedStateRef = useRef(false)
  const dragStartRef = useRef<{ pointerX: number; startPercent: number } | null>(null)
  const verticalDragStartRef = useRef<{ pointerY: number; startPercent: number } | null>(null)
  const splitContainerRef = useRef<HTMLDivElement>(null)
  const splitHandleRef = useRef<HTMLDivElement>(null)
  const desktopSplitStorageKey = 'axon.web.pulse.editor-split.desktop'
  const mobileSplitStorageKey = 'axon.web.pulse.editor-split.mobile'

  useEffect(() => {
    chatHistoryRef.current = chatHistory
  }, [chatHistory])

  useEffect(() => {
    if (!selectedFile || !markdownContent) return
    setDocumentMarkdown(markdownContent)
    const parts = selectedFile.split('/')
    setDocumentTitle(parts[parts.length - 1] ?? selectedFile)
  }, [markdownContent, selectedFile])

  useEffect(() => {
    try {
      const restored = parsePersistedWorkspaceState(
        window.localStorage.getItem(PULSE_WORKSPACE_STATE_KEY),
      )
      if (!restored) {
        hasHydratedPersistedStateRef.current = true
        return
      }
      setPulsePermissionLevel(restored.permissionLevel)
      setPulseModel(restored.model)
      setDocumentMarkdown(restored.documentMarkdown)
      setChatHistory(restored.chatHistory)
      setDocumentTitle(restored.documentTitle)
      setChatSessionId(restored.chatSessionId)
      setIndexedSources(restored.indexedSources)
      setActiveThreadSources(restored.activeThreadSources)
      setDesktopSplitPercent(restored.desktopSplitPercent)
      setMobileSplitPercent(restored.mobileSplitPercent)
      setLastResponseLatencyMs(restored.lastResponseLatencyMs)
      setLastResponseModel(restored.lastResponseModel)
      setDesktopViewMode(restored.desktopViewMode)
      setDesktopPaneOrder(restored.desktopPaneOrder)
      messageIdRef.current = restored.chatHistory.length
    } catch {
      // Ignore persistence restore failures.
    } finally {
      hasHydratedPersistedStateRef.current = true
    }
  }, [])

  // Capture context from scrape/crawl commands so Claude knows what the user indexed.
  useEffect(() => {
    return subscribe((msg: WsServerMsg) => {
      // Track the current command's mode and input URL.
      if (msg.type === 'command.start') {
        lastCmdModeRef.current = msg.data.ctx.mode
        lastCmdInputRef.current = msg.data.ctx.input
      }
      // Scrape: inject the full markdown directly into the system prompt.
      if (msg.type === 'command.output.json' && msg.data.ctx.mode === 'scrape') {
        const data = msg.data.data as Record<string, unknown> | null
        if (data && typeof data.markdown === 'string' && data.markdown.length > 0) {
          setScrapedContext({
            url: typeof data.url === 'string' ? data.url : lastCmdInputRef.current,
            markdown: data.markdown,
          })
        }
      }
      // Crawl: record the URL so the system prompt tells Claude where content is indexed.
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

  useEffect(() => {
    desktopSplitPercentRef.current = desktopSplitPercent
  }, [desktopSplitPercent])

  useEffect(() => {
    mobileSplitPercentRef.current = mobileSplitPercent
  }, [mobileSplitPercent])

  const persistWorkspaceState = useCallback(() => {
    if (!hasHydratedPersistedStateRef.current) return
    try {
      const payload: PersistedPulseWorkspaceState = {
        permissionLevel,
        model,
        documentMarkdown,
        chatHistory: chatHistory.slice(-250),
        documentTitle,
        chatSessionId,
        indexedSources: indexedSources.slice(-50),
        activeThreadSources: activeThreadSources.slice(-50),
        desktopSplitPercent,
        mobileSplitPercent,
        lastResponseLatencyMs,
        lastResponseModel,
        desktopViewMode,
        desktopPaneOrder,
        savedAt: Date.now(),
      }
      window.localStorage.setItem(PULSE_WORKSPACE_STATE_KEY, JSON.stringify(payload))
    } catch {
      // Ignore persistence write failures.
    }
  }, [
    permissionLevel,
    model,
    documentMarkdown,
    chatHistory,
    documentTitle,
    chatSessionId,
    indexedSources,
    activeThreadSources,
    desktopSplitPercent,
    mobileSplitPercent,
    lastResponseLatencyMs,
    lastResponseModel,
    desktopViewMode,
    desktopPaneOrder,
  ])

  useEffect(() => {
    persistWorkspaceState()
  }, [persistWorkspaceState])

  useEffect(() => {
    const flushState = () => persistWorkspaceState()
    const onVisibility = () => {
      if (document.visibilityState === 'hidden') flushState()
    }
    window.addEventListener('pagehide', flushState)
    document.addEventListener('visibilitychange', onVisibility)
    return () => {
      window.removeEventListener('pagehide', flushState)
      document.removeEventListener('visibilitychange', onVisibility)
    }
  }, [persistWorkspaceState])

  const createMessage = useCallback(
    (partial: Omit<ChatMessage, 'id' | 'createdAt'>): ChatMessage => {
      messageIdRef.current += 1
      const now = Date.now()
      return {
        ...partial,
        id: `msg-${now}-${messageIdRef.current}`,
        createdAt: now,
      }
    },
    [],
  )

  const applyOperations = useCallback((ops: DocOperation[]) => {
    setDocumentMarkdown((prev) => {
      let next = prev
      for (const op of ops) {
        switch (op.type) {
          case 'replace_document':
            next = op.markdown
            break
          case 'append_markdown':
            next = `${next}\n\n${op.markdown}`
            break
          case 'insert_section':
            next =
              op.position === 'top'
                ? `## ${op.heading}\n\n${op.markdown}\n\n${next}`
                : `${next}\n\n## ${op.heading}\n\n${op.markdown}`
            break
        }
      }
      return next
    })
  }, [])

  const runChatPrompt = useCallback(
    async (
      prompt: string,
      conversationHistory: Array<{ role: 'user' | 'assistant'; content: string }>,
      signal: AbortSignal,
    ) => {
      const response = await fetch('/api/pulse/chat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        signal,
        body: JSON.stringify({
          prompt,
          sessionId: chatSessionId ?? undefined,
          documentMarkdown,
          selectedCollections: ['cortex'],
          threadSources: activeThreadSources,
          scrapedContext: scrapedContext ?? undefined,
          conversationHistory,
          permissionLevel,
          model,
        }),
      })
      if (!response.ok) {
        const errorBody = await response.text()
        let detail = ''
        if (errorBody) {
          try {
            const parsed = JSON.parse(errorBody) as { error?: unknown; message?: unknown }
            detail =
              typeof parsed.error === 'string'
                ? parsed.error
                : typeof parsed.message === 'string'
                  ? parsed.message
                  : errorBody
          } catch {
            detail = errorBody
          }
        }
        const suffix = detail ? `: ${detail}` : ''
        throw new Error(`Pulse chat failed (${response.status})${suffix}`)
      }
      return (await response.json()) as PulseChatResponse
    },
    [activeThreadSources, chatSessionId, documentMarkdown, model, permissionLevel, scrapedContext],
  )

  const runSourcePrompt = useCallback(async (urls: string[], signal: AbortSignal) => {
    const response = await fetch('/api/pulse/source', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      signal,
      body: JSON.stringify({ urls }),
    })
    if (!response.ok) {
      const body = await response.text()
      throw new Error(body || `Source ingest failed (${response.status})`)
    }
    return (await response.json()) as PulseSourceResponse
  }, [])

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
          // Inject scraped markdown directly so Claude knows the content
          // without waiting for RAG retrieval.
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
        const data = await runChatPrompt(boundedPrompt, conversationHistory, controller.signal)
        if (inFlightPromptRef.current !== promptId) return
        if (data.sessionId) {
          setChatSessionId(data.sessionId)
        }
        if (data.metadata) {
          setLastResponseLatencyMs(data.metadata.elapsedMs)
          setLastResponseModel(data.metadata.model)
          setLastContextStats({
            promptChars: data.metadata.promptChars,
            documentChars: data.metadata.documentChars,
            conversationChars: data.metadata.conversationChars,
            citationChars: data.metadata.citationChars,
            contextCharsTotal: data.metadata.contextCharsTotal,
            contextBudgetChars: data.metadata.contextBudgetChars,
          })
        }
        setChatHistory((prev) => [
          ...prev,
          createMessage({
            role: 'assistant',
            content: data.text,
            citations: data.citations,
            operations: data.operations,
            toolUses: data.toolUses,
            blocks: data.blocks,
          }),
        ])

        if (data.operations.length > 0) {
          const permission = checkPermission(permissionLevel, data.operations, {
            isCurrentDoc: true,
            currentDocMarkdown: documentMarkdown,
          })

          if (permission.allowed && !permission.requiresConfirmation) {
            applyOperations(data.operations)
          } else if (permission.allowed && permission.requiresConfirmation) {
            const validation = validateDocOperations(data.operations, documentMarkdown)
            setPendingOps(data.operations)
            setPendingValidation(validation)
          }
        }
      } catch (err: unknown) {
        if (err instanceof Error && err.name === 'AbortError') return
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
        if (activePromptAbortRef.current === controller) {
          activePromptAbortRef.current = null
        }
        if (inFlightPromptRef.current === promptId) {
          setIsChatLoading(false)
        }
      }
    },
    [
      applyOperations,
      createMessage,
      documentMarkdown,
      permissionLevel,
      runChatPrompt,
      runSourcePrompt,
    ],
  )

  useEffect(() => {
    updateWorkspaceContext({
      turns: chatHistory.length,
      sourceCount: indexedSources.length,
      threadSourceCount: activeThreadSources.length,
      promptChars: lastContextStats?.promptChars ?? workspacePrompt?.length ?? 0,
      documentChars: lastContextStats?.documentChars ?? documentMarkdown.length,
      conversationChars:
        lastContextStats?.conversationChars ??
        chatHistory.reduce((total, message) => total + message.content.length, 0),
      citationChars: lastContextStats?.citationChars ?? 0,
      contextCharsTotal:
        lastContextStats?.contextCharsTotal ??
        (workspacePrompt?.length ?? 0) +
          documentMarkdown.length +
          chatHistory.reduce((total, message) => total + message.content.length, 0),
      contextBudgetChars:
        lastContextStats?.contextBudgetChars ??
        (model === 'opus' ? 200_000 : model === 'haiku' ? 80_000 : 120_000),
      lastLatencyMs: lastResponseLatencyMs ?? 0,
      model,
      permissionLevel,
      saveStatus,
    })
  }, [
    activeThreadSources.length,
    chatHistory,
    chatHistory.length,
    documentMarkdown.length,
    indexedSources.length,
    lastResponseLatencyMs,
    lastContextStats,
    model,
    permissionLevel,
    saveStatus,
    updateWorkspaceContext,
    workspacePrompt,
  ])

  useEffect(() => {
    return () => updateWorkspaceContext(null)
  }, [updateWorkspaceContext])

  useEffect(() => {
    const media = window.matchMedia('(min-width: 1024px)')
    const update = () => setIsDesktop(media.matches)
    update()
    media.addEventListener('change', update)
    return () => media.removeEventListener('change', update)
  }, [])

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!event.altKey) return
      const key = event.key
      if (key !== '1' && key !== '2' && key !== '3') return
      event.preventDefault()
      if (event.shiftKey) {
        const permissionByIndex: PulsePermissionLevel[] = [
          'plan',
          'accept-edits',
          'bypass-permissions',
        ]
        setPulsePermissionLevel(permissionByIndex[Number(key) - 1] ?? 'accept-edits')
        return
      }
      const modelByIndex: PulseModel[] = ['sonnet', 'opus', 'haiku']
      setPulseModel(modelByIndex[Number(key) - 1] ?? 'sonnet')
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])

  useEffect(() => {
    try {
      const desktop = window.localStorage.getItem(desktopSplitStorageKey)
      const mobile = window.localStorage.getItem(mobileSplitStorageKey)
      const parsedDesktop = Number(desktop)
      const parsedMobile = Number(mobile)
      if (Number.isFinite(parsedDesktop) && parsedDesktop >= 42 && parsedDesktop <= 74) {
        setDesktopSplitPercent(parsedDesktop)
      }
      if (Number.isFinite(parsedMobile) && parsedMobile >= 35 && parsedMobile <= 70) {
        setMobileSplitPercent(parsedMobile)
      }
    } catch {
      // Ignore storage errors.
    }
  }, [])

  useEffect(() => {
    function onPointerMove(event: PointerEvent) {
      const start = dragStartRef.current
      const container = splitContainerRef.current
      if (!start || !container) return
      const rect = container.getBoundingClientRect()
      if (rect.width <= 0) return
      const deltaPx = event.clientX - start.pointerX
      const deltaPercent = (deltaPx / rect.width) * 100
      const next = Math.max(42, Math.min(74, start.startPercent + deltaPercent))
      setDesktopSplitPercent(next)
    }

    function stopDrag() {
      if (!dragStartRef.current) return
      dragStartRef.current = null
      splitHandleRef.current?.classList.remove('bg-[rgba(175,215,255,0.3)]')
      try {
        window.localStorage.setItem(desktopSplitStorageKey, String(desktopSplitPercentRef.current))
      } catch {
        // Ignore storage errors.
      }
    }

    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', stopDrag)
    return () => {
      window.removeEventListener('pointermove', onPointerMove)
      window.removeEventListener('pointerup', stopDrag)
    }
  }, [])

  useEffect(() => {
    function onPointerMove(event: PointerEvent) {
      const start = verticalDragStartRef.current
      const container = splitContainerRef.current
      if (!start || !container) return
      const rect = container.getBoundingClientRect()
      if (rect.height <= 0) return
      const deltaPx = event.clientY - start.pointerY
      const deltaPercent = (deltaPx / rect.height) * 100
      const next = Math.max(35, Math.min(70, start.startPercent + deltaPercent))
      setMobileSplitPercent(next)
    }

    function stopVerticalDrag() {
      if (!verticalDragStartRef.current) return
      verticalDragStartRef.current = null
      try {
        window.localStorage.setItem(mobileSplitStorageKey, String(mobileSplitPercentRef.current))
      } catch {
        // Ignore storage errors.
      }
    }

    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', stopVerticalDrag)
    return () => {
      window.removeEventListener('pointermove', onPointerMove)
      window.removeEventListener('pointerup', stopVerticalDrag)
    }
  }, [])

  useEffect(() => {
    if (workspacePromptVersion === 0) {
      lastHandledPromptVersionRef.current = 0
      return
    }
    if (!workspacePrompt) return
    if (workspacePromptVersion <= lastHandledPromptVersionRef.current) return
    lastHandledPromptVersionRef.current = workspacePromptVersion

    void handlePrompt(workspacePrompt)
  }, [workspacePromptVersion, workspacePrompt, handlePrompt])

  useEffect(() => {
    if (!documentMarkdown || !documentTitle) return

    if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current)
    const snapshot = `${documentTitle}\n---\n${documentMarkdown}`
    if (snapshot === lastSavedSnapshotRef.current) return
    autosaveTimerRef.current = setTimeout(() => {
      void (async () => {
        if (autosaveAbortRef.current) {
          autosaveAbortRef.current.abort()
        }
        const controller = new AbortController()
        autosaveAbortRef.current = controller
        try {
          setSaveStatus('saving')
          const response = await fetch('/api/pulse/save', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            signal: controller.signal,
            body: JSON.stringify({
              title: documentTitle,
              markdown: documentMarkdown,
              embed: true,
            }),
          })
          if (response.ok) {
            lastSavedSnapshotRef.current = snapshot
            setSaveStatus('saved')
          } else {
            setSaveStatus('error')
          }
          setTimeout(() => setSaveStatus('idle'), 2000)
        } catch (error: unknown) {
          if (error instanceof Error && error.name === 'AbortError') return
          setSaveStatus('error')
        } finally {
          if (autosaveAbortRef.current === controller) {
            autosaveAbortRef.current = null
          }
        }
      })()
    }, 1500)

    return () => {
      if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current)
    }
  }, [documentMarkdown, documentTitle])

  useEffect(() => {
    return () => {
      if (autosaveAbortRef.current) {
        autosaveAbortRef.current.abort()
        autosaveAbortRef.current = null
      }
      if (activePromptAbortRef.current) {
        activePromptAbortRef.current.abort()
        activePromptAbortRef.current = null
      }
    }
  }, [])

  return (
    <div className="mt-1 space-y-1.5">
      <PulseToolbar
        title={documentTitle}
        onTitleChange={setDocumentTitle}
        isDesktop={isDesktop}
        desktopViewMode={desktopViewMode}
        onDesktopViewModeChange={setDesktopViewMode}
        desktopPaneOrder={desktopPaneOrder}
        onSwapPanes={() =>
          setDesktopPaneOrder((prev) => (prev === 'editor-first' ? 'chat-first' : 'editor-first'))
        }
        contextCharsTotal={
          lastContextStats?.contextCharsTotal ??
          (workspacePrompt?.length ?? 0) +
            documentMarkdown.length +
            chatHistory.reduce((sum, m) => sum + m.content.length, 0)
        }
        contextBudgetChars={
          lastContextStats?.contextBudgetChars ??
          (model === 'opus' ? 200_000 : model === 'haiku' ? 80_000 : 120_000)
        }
      />
      <div className="flex h-[58vh] overflow-hidden rounded-xl border border-[rgba(255,135,175,0.1)] bg-[rgba(10,18,35,0.42)] lg:h-[68vh]">
        {crawlFiles.length > 0 && (
          <CrawlFileExplorer
            files={crawlFiles}
            selectedFile={selectedFile}
            onSelectFile={selectFile}
            jobId={currentJobId}
          />
        )}
        <div
          ref={splitContainerRef}
          className="flex h-full min-w-0 flex-1 flex-col gap-1.5 p-1.5 lg:flex-row lg:gap-1.5"
        >
          <div
            className={`${
              isDesktop
                ? desktopViewMode === 'editor' || desktopViewMode === 'both'
                  ? 'flex'
                  : 'hidden'
                : mobilePane === 'editor'
                  ? 'flex'
                  : 'hidden'
            } min-w-0 overflow-hidden rounded-xl border border-[rgba(255,135,175,0.1)] bg-[rgba(10,18,35,0.5)] lg:flex-none`}
            style={{
              flexBasis: `${isDesktop ? desktopSplitPercent : mobileSplitPercent}%`,
              order: isDesktop ? (desktopPaneOrder === 'editor-first' ? 1 : 3) : 2,
            }}
          >
            <PulseEditorPane
              markdown={documentMarkdown}
              onMarkdownChange={setDocumentMarkdown}
              scrollStorageKey="axon.web.pulse.editor-scroll"
            />
          </div>
          <div
            ref={splitHandleRef}
            role="separator"
            aria-orientation="vertical"
            className={`w-2 cursor-col-resize rounded bg-[rgba(255,135,175,0.14)] transition-colors hover:bg-[rgba(175,215,255,0.2)] ${desktopViewMode === 'both' ? 'hidden lg:block' : 'hidden'}`}
            style={{ order: isDesktop ? 2 : 2 }}
            onPointerDown={(event) => {
              dragStartRef.current = { pointerX: event.clientX, startPercent: desktopSplitPercent }
              splitHandleRef.current?.classList.add('bg-[rgba(175,215,255,0.3)]')
            }}
          />
          <div
            className={`${
              isDesktop
                ? desktopViewMode === 'chat' || desktopViewMode === 'both'
                  ? 'flex'
                  : 'hidden'
                : mobilePane === 'chat'
                  ? 'flex'
                  : 'hidden'
            } min-h-0 min-w-0 flex-col overflow-hidden rounded-xl border border-[rgba(255,135,175,0.12)] bg-[rgba(10,18,35,0.52)] lg:flex lg:flex-1`}
            style={{ order: isDesktop ? (desktopPaneOrder === 'editor-first' ? 3 : 1) : 1 }}
          >
            <PulseChatPane
              messages={chatHistory}
              isLoading={isChatLoading}
              indexedSources={indexedSources}
              activeThreadSources={activeThreadSources}
              onRemoveSource={(url) =>
                setActiveThreadSources((prev) => prev.filter((existingUrl) => existingUrl !== url))
              }
              onRetry={(prompt) => void handlePrompt(prompt)}
              mobilePane={mobilePane}
              onMobilePaneChange={setMobilePane}
              isDesktop={isDesktop}
              requestNotice={requestNotice}
            />
            {pendingOps && pendingValidation && (
              <div className="p-3">
                <PulseOpConfirmation
                  operations={pendingOps}
                  validation={pendingValidation}
                  onConfirm={() => {
                    applyOperations(pendingOps)
                    setPendingOps(null)
                    setPendingValidation(null)
                  }}
                  onReject={() => {
                    setPendingOps(null)
                    setPendingValidation(null)
                  }}
                />
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
