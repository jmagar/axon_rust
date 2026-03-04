'use client'

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { usePulseAutosave } from '@/hooks/use-pulse-autosave'
import { usePulseChat } from '@/hooks/use-pulse-chat'
import { usePulsePersistence } from '@/hooks/use-pulse-persistence'
import { usePulseSettings } from '@/hooks/use-pulse-settings'
import { useSplitPane } from '@/hooks/use-split-pane'
import {
  useWsExecutionState,
  useWsMessageActions,
  useWsWorkspaceState,
} from '@/hooks/use-ws-messages'
import type { ValidationResult } from '@/lib/pulse/doc-ops'
import type { DocOperation, PulseModel, PulsePermissionLevel } from '@/lib/pulse/types'
import { PULSE_WORKSPACE_STATE_KEY } from '@/lib/pulse/workspace-persistence'

/**
 * Encapsulates all behavioral wiring for the Pulse workspace:
 * document state, chat, persistence, autosave, keyboard shortcuts, and layout.
 *
 * The PulseWorkspace component consumes this hook and handles only layout/rendering.
 */
export function usePulseWorkspaceBehavior() {
  const { selectedFile, markdownContent } = useWsExecutionState()
  const { workspacePrompt, workspacePromptVersion, pulseModel, pulsePermissionLevel } =
    useWsWorkspaceState()
  const { updateWorkspaceContext, setPulseModel, setPulsePermissionLevel } = useWsMessageActions()
  const { subscribe } = useAxonWs()

  const [documentMarkdown, setDocumentMarkdown] = useState('')
  const [documentTitle, setDocumentTitle] = useState('Untitled')
  const [currentDocFilename, setCurrentDocFilename] = useState<string | null>(null)
  const [pendingOps, setPendingOps] = useState<DocOperation[] | null>(null)
  const [pendingValidation, setPendingValidation] = useState<ValidationResult | null>(null)
  const [sourcesExpanded, setSourcesExpanded] = useState(false)

  const model = pulseModel
  const permissionLevel = pulsePermissionLevel

  const lastHandledPromptVersionRef = useRef(0)
  const { settings: pulseSettings } = usePulseSettings()

  const splitPane = useSplitPane()

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

  const chat = usePulseChat({
    documentMarkdown,
    permissionLevel,
    model,
    subscribe,
    onApplyOperations: applyOperations,
    onPendingOps: setPendingOps,
    onPendingValidation: setPendingValidation,
    effort: pulseSettings.effort,
    maxTurns: pulseSettings.maxTurns,
    maxBudgetUsd: pulseSettings.maxBudgetUsd,
    appendSystemPrompt: pulseSettings.appendSystemPrompt,
    disableSlashCommands: pulseSettings.disableSlashCommands,
    noSessionPersistence: pulseSettings.noSessionPersistence,
    fallbackModel: pulseSettings.fallbackModel,
    allowedTools: pulseSettings.allowedTools,
    disallowedTools: pulseSettings.disallowedTools,
  })

  const {
    chatHistory,
    isChatLoading,
    streamPhase,
    liveToolUses,
    requestNotice,
    handlePrompt,
    handleCancelPrompt,
    indexedSources,
    activeThreadSources,
    setActiveThreadSources,
    setChatHistory,
    chatSessionId,
    setChatSessionId,
    setIndexedSources,
    lastResponseLatencyMs,
    lastResponseModel,
    lastContextStats,
    messageIdRef,
    setLastResponseLatencyMs,
    setLastResponseModel,
  } = chat

  const latestCitationCount = useMemo(() => {
    for (let i = chatHistory.length - 1; i >= 0; i -= 1) {
      const msg = chatHistory[i]
      if (msg.role === 'assistant' && msg.citations && msg.citations.length > 0) {
        return msg.citations.length
      }
    }
    return 0
  }, [chatHistory])

  const handleNewSession = useCallback(() => {
    handleCancelPrompt()
    setChatHistory([])
    setDocumentMarkdown('')
    setDocumentTitle('Untitled')
    setCurrentDocFilename(null)
    setChatSessionId(null)
    setIndexedSources([])
    setActiveThreadSources([])
    try {
      window.localStorage.removeItem(PULSE_WORKSPACE_STATE_KEY)
    } catch {
      // Ignore storage errors.
    }
  }, [
    handleCancelPrompt,
    setChatHistory,
    setChatSessionId,
    setIndexedSources,
    setActiveThreadSources,
  ])

  usePulsePersistence({
    data: {
      permissionLevel,
      model,
      documentMarkdown,
      chatHistory,
      documentTitle,
      currentDocFilename,
      chatSessionId,
      indexedSources,
      activeThreadSources,
      desktopSplitPercent: splitPane.desktopSplitPercent,
      mobileSplitPercent: splitPane.mobileSplitPercent,
      lastResponseLatencyMs,
      lastResponseModel,
      showChat: splitPane.showChat,
      showEditor: splitPane.showEditor,
    },
    setters: {
      setPulsePermissionLevel,
      setPulseModel,
      setDocumentMarkdown,
      setChatHistory,
      setDocumentTitle,
      setCurrentDocFilename,
      setChatSessionId,
      setIndexedSources,
      setActiveThreadSources,
      setDesktopSplitPercent: splitPane.setDesktopSplitPercent,
      setMobileSplitPercent: splitPane.setMobileSplitPercent,
      setLastResponseLatencyMs,
      setLastResponseModel,
      setShowChat: splitPane.setShowChat,
      setShowEditor: splitPane.setShowEditor,
    },
    messageIdRef,
  })

  const { saveStatus, savedFilename } = usePulseAutosave(
    documentMarkdown,
    documentTitle,
    currentDocFilename,
  )

  // Sync savedFilename back to currentDocFilename after the first save creates the file
  useEffect(() => {
    if (savedFilename && !currentDocFilename) {
      setCurrentDocFilename(savedFilename)
    }
  }, [savedFilename, currentDocFilename])

  // File selection effect
  useEffect(() => {
    if (!selectedFile || markdownContent == null) return
    setDocumentMarkdown(markdownContent)
    const parts = selectedFile.split('/')
    setDocumentTitle(parts[parts.length - 1] ?? selectedFile)
    if (selectedFile.includes('/.cache/pulse/')) {
      const basename = parts[parts.length - 1] ?? null
      setCurrentDocFilename(basename)
    } else {
      setCurrentDocFilename(null)
    }
  }, [markdownContent, selectedFile])

  // Update workspace context effect
  useEffect(() => {
    updateWorkspaceContext({
      turns: chatHistory.length,
      sourceCount: indexedSources.length,
      threadSourceCount: activeThreadSources.length,
      contextCharsTotal: lastContextStats?.contextCharsTotal ?? 0,
      contextBudgetChars: lastContextStats?.contextBudgetChars ?? 0,
      lastLatencyMs: lastResponseLatencyMs ?? 0,
      model,
      permissionLevel,
      saveStatus,
    })
  }, [
    activeThreadSources.length,
    chatHistory.length,
    indexedSources.length,
    lastResponseLatencyMs,
    lastContextStats,
    model,
    permissionLevel,
    saveStatus,
    updateWorkspaceContext,
  ])

  // Cleanup workspace context on unmount
  useEffect(() => {
    return () => updateWorkspaceContext(null)
  }, [updateWorkspaceContext])

  // Keyboard shortcuts — model/permission hotkeys + layout toggles
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.altKey) {
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
        return
      }
      const isMod = event.metaKey || event.ctrlKey
      if (isMod && event.key === 'b') {
        event.preventDefault()
        document.dispatchEvent(new CustomEvent('axon:sidebar:toggle'))
        return
      }
      if (isMod && event.shiftKey && event.key === 'E') {
        event.preventDefault()
        splitPane.toggleEditor()
        return
      }
      if (isMod && event.shiftKey && event.key === 'C') {
        event.preventDefault()
        splitPane.toggleChat()
        return
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [setPulseModel, setPulsePermissionLevel, splitPane.toggleChat, splitPane.toggleEditor])

  // Workspace prompt handler
  const handlePromptRef = useRef(handlePrompt)
  useEffect(() => {
    handlePromptRef.current = handlePrompt
  }, [handlePrompt])

  useEffect(() => {
    if (workspacePromptVersion === 0) {
      lastHandledPromptVersionRef.current = 0
      return
    }
    if (!workspacePrompt) return
    if (workspacePromptVersion <= lastHandledPromptVersionRef.current) return
    lastHandledPromptVersionRef.current = workspacePromptVersion

    void handlePromptRef.current(workspacePrompt)

    return () => {
      if (lastHandledPromptVersionRef.current === workspacePromptVersion) {
        lastHandledPromptVersionRef.current = workspacePromptVersion - 1
      }
    }
  }, [workspacePromptVersion, workspacePrompt])

  return {
    // Document state
    documentMarkdown,
    setDocumentMarkdown,
    documentTitle,
    setDocumentTitle,
    currentDocFilename,
    sourcesExpanded,
    setSourcesExpanded,
    pendingOps,
    setPendingOps,
    pendingValidation,
    setPendingValidation,
    applyOperations,

    // Chat (re-exported)
    chatHistory,
    isChatLoading,
    streamPhase,
    liveToolUses,
    requestNotice,
    handlePrompt,
    handleCancelPrompt,
    indexedSources,
    activeThreadSources,
    setActiveThreadSources,
    latestCitationCount,
    handleNewSession,

    // Layout (re-exported from useSplitPane)
    ...splitPane,
  }
}
