'use client'

import type React from 'react'
import { useCallback, useEffect, useRef } from 'react'
import type { PulseAgent, PulseModel, PulsePermissionLevel } from '@/lib/pulse/types'
import type { ChatMessage } from '@/lib/pulse/workspace-persistence'
import {
  buildPersistedPayload,
  PULSE_WORKSPACE_STATE_KEY,
  parsePersistedWorkspaceState,
} from '@/lib/pulse/workspace-persistence'

interface PersistenceData {
  permissionLevel: PulsePermissionLevel
  agent: PulseAgent
  model: PulseModel
  documentMarkdown: string
  chatHistory: ChatMessage[]
  documentTitle: string
  currentDocFilename: string | null
  chatSessionId: string | null
  indexedSources: string[]
  activeThreadSources: string[]
  desktopSplitPercent: number
  mobileSplitPercent: number
  lastResponseLatencyMs: number | null
  lastResponseModel: PulseModel | null
  showChat: boolean
  showEditor: boolean
}

interface PersistenceSetters {
  setPulsePermissionLevel: (v: PulsePermissionLevel) => void
  setPulseAgent: (v: PulseAgent) => void
  setPulseModel: (v: PulseModel) => void
  setDocumentMarkdown: (v: string) => void
  setChatHistory: (v: ChatMessage[]) => void
  setDocumentTitle: (v: string) => void
  setCurrentDocFilename: (v: string | null) => void
  setChatSessionId: (v: string | null) => void
  setIndexedSources: (v: string[]) => void
  setActiveThreadSources: (v: string[]) => void
  setDesktopSplitPercent: (v: number) => void
  setMobileSplitPercent: (v: number) => void
  setLastResponseLatencyMs: (v: number | null) => void
  setLastResponseModel: (v: PulseModel | null) => void
  setShowChat: (v: boolean) => void
  setShowEditor: (v: boolean) => void
}

interface UsePulsePersistenceInput {
  data: PersistenceData
  setters: PersistenceSetters
  messageIdRef: React.MutableRefObject<number>
}

export function usePulsePersistence({ data, setters, messageIdRef }: UsePulsePersistenceInput) {
  const {
    permissionLevel,
    agent,
    model,
    documentMarkdown,
    chatHistory,
    documentTitle,
    currentDocFilename,
    chatSessionId,
    indexedSources,
    activeThreadSources,
    desktopSplitPercent,
    mobileSplitPercent,
    lastResponseLatencyMs,
    lastResponseModel,
    showChat,
    showEditor,
  } = data
  const {
    setPulsePermissionLevel,
    setPulseAgent,
    setPulseModel,
    setDocumentMarkdown,
    setChatHistory,
    setDocumentTitle,
    setCurrentDocFilename,
    setChatSessionId,
    setIndexedSources,
    setActiveThreadSources,
    setDesktopSplitPercent,
    setMobileSplitPercent,
    setLastResponseLatencyMs,
    setLastResponseModel,
    setShowChat,
    setShowEditor,
  } = setters
  const hasHydratedRef = useRef(false)
  const persistFnRef = useRef<(() => void) | null>(null)
  const persistTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Hydration effect — reads from localStorage, calls all setters
  useEffect(() => {
    if (hasHydratedRef.current) return
    try {
      const restored = parsePersistedWorkspaceState(
        window.localStorage.getItem(PULSE_WORKSPACE_STATE_KEY),
      )
      if (!restored) {
        hasHydratedRef.current = true
        return
      }
      setPulsePermissionLevel(restored.permissionLevel)
      setPulseAgent(restored.agent)
      setPulseModel(restored.model)
      setDocumentMarkdown(restored.documentMarkdown)
      setChatHistory(restored.chatHistory)
      setDocumentTitle(restored.documentTitle)
      setCurrentDocFilename(restored.currentDocFilename)
      setChatSessionId(restored.chatSessionId)
      setIndexedSources(restored.indexedSources)
      setActiveThreadSources(restored.activeThreadSources)
      setDesktopSplitPercent(restored.desktopSplitPercent)
      setMobileSplitPercent(restored.mobileSplitPercent)
      setLastResponseLatencyMs(restored.lastResponseLatencyMs)
      setLastResponseModel(restored.lastResponseModel)
      setShowChat(restored.showChat)
      setShowEditor(restored.showEditor)
      messageIdRef.current = restored.chatHistory.length
    } catch {
      // Ignore persistence restore failures.
    } finally {
      hasHydratedRef.current = true
    }
  }, [
    messageIdRef,
    setActiveThreadSources,
    setChatHistory,
    setChatSessionId,
    setCurrentDocFilename,
    setDesktopSplitPercent,
    setDocumentMarkdown,
    setShowChat,
    setShowEditor,
    setDocumentTitle,
    setIndexedSources,
    setLastResponseLatencyMs,
    setLastResponseModel,
    setMobileSplitPercent,
    setPulseModel,
    setPulsePermissionLevel,
    setPulseAgent,
  ])

  const persistWorkspaceState = useCallback(() => {
    if (!hasHydratedRef.current) return
    try {
      const payload = buildPersistedPayload({
        permissionLevel,
        agent,
        model,
        documentMarkdown,
        chatHistory,
        documentTitle,
        currentDocFilename,
        chatSessionId,
        indexedSources,
        activeThreadSources,
        desktopSplitPercent,
        mobileSplitPercent,
        lastResponseLatencyMs,
        lastResponseModel,
        showChat,
        showEditor,
      })
      window.localStorage.setItem(PULSE_WORKSPACE_STATE_KEY, JSON.stringify(payload))
    } catch {
      // Ignore persistence write failures.
    }
  }, [
    activeThreadSources,
    chatHistory,
    chatSessionId,
    currentDocFilename,
    desktopSplitPercent,
    documentMarkdown,
    showChat,
    showEditor,
    documentTitle,
    indexedSources,
    lastResponseLatencyMs,
    lastResponseModel,
    mobileSplitPercent,
    agent,
    model,
    permissionLevel,
  ])

  // Keep ref pointing at the latest persist function (avoids stale closures)
  persistFnRef.current = persistWorkspaceState

  // Auto-persist effect — debounced to 2s to avoid serialization storm during streaming
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentional — we use persistFnRef to avoid stale closures; the dep triggers the debounce cycle
  useEffect(() => {
    if (persistTimerRef.current) clearTimeout(persistTimerRef.current)
    persistTimerRef.current = setTimeout(() => persistFnRef.current?.(), 2000)
  }, [persistWorkspaceState])

  // Cleanup debounce timer on unmount
  useEffect(() => {
    return () => {
      if (persistTimerRef.current) clearTimeout(persistTimerRef.current)
    }
  }, [])

  // Pagehide/visibilitychange effect — immediate flush (no debounce)
  useEffect(() => {
    const flushState = () => persistFnRef.current?.()
    const onVisibility = () => {
      if (document.visibilityState === 'hidden') flushState()
    }
    window.addEventListener('pagehide', flushState)
    document.addEventListener('visibilitychange', onVisibility)
    return () => {
      window.removeEventListener('pagehide', flushState)
      document.removeEventListener('visibilitychange', onVisibility)
    }
  }, [])
}
