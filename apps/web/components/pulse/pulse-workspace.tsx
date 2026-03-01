'use client'

import { BookOpen, ChevronDown, ChevronLeft, ChevronRight } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { usePulseAutosave } from '@/hooks/use-pulse-autosave'
import { usePulseChat } from '@/hooks/use-pulse-chat'
import { usePulsePersistence } from '@/hooks/use-pulse-persistence'
import { usePulseSettings } from '@/hooks/use-pulse-settings'
import { useSplitPane } from '@/hooks/use-split-pane'
import { useWsMessages } from '@/hooks/use-ws-messages'
import type { ValidationResult } from '@/lib/pulse/doc-ops'
import type { DocOperation, PulseModel, PulsePermissionLevel } from '@/lib/pulse/types'
import { PULSE_WORKSPACE_STATE_KEY } from '@/lib/pulse/workspace-persistence'
import { PulseChatPane } from './pulse-chat-pane'
import { PulseEditorPane } from './pulse-editor-pane'
import { PulseMobilePaneSwitcher } from './pulse-mobile-pane-switcher'
import { PulseOpConfirmation } from './pulse-op-confirmation'
import { PulseToolbar } from './pulse-toolbar'

export function PulseWorkspace() {
  const {
    workspacePrompt,
    workspacePromptVersion,
    updateWorkspaceContext,
    pulseModel,
    pulsePermissionLevel,
    setPulseModel,
    setPulsePermissionLevel,
    selectedFile,
    markdownContent,
  } = useWsMessages()
  const { subscribe } = useAxonWs()

  const [documentMarkdown, setDocumentMarkdown] = useState('')
  const [documentTitle, setDocumentTitle] = useState('Untitled')
  const [currentDocFilename, setCurrentDocFilename] = useState<string | null>(null)
  const [pendingOps, setPendingOps] = useState<DocOperation[] | null>(null)
  const [pendingValidation, setPendingValidation] = useState<ValidationResult | null>(null)
  const [sourcesExpanded, setSourcesExpanded] = useState(false)

  const model = pulseModel as PulseModel
  const permissionLevel = pulsePermissionLevel as PulsePermissionLevel

  const lastHandledPromptVersionRef = useRef(0)
  const { settings: pulseSettings } = usePulseSettings()

  const {
    desktopSplitPercent,
    setDesktopSplitPercent,
    mobileSplitPercent,
    setMobileSplitPercent,
    isDesktop,
    mobilePane,
    setMobilePane,
    showChat,
    setShowChat,
    toggleChat,
    showEditor,
    setShowEditor,
    toggleEditor,
    splitContainerRef,
    splitHandleRef,
    dragStartRef,
  } = useSplitPane()

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

  const {
    chatHistory,
    setChatHistory,
    isChatLoading,
    chatSessionId,
    setChatSessionId,
    indexedSources,
    setIndexedSources,
    activeThreadSources,
    setActiveThreadSources,
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
  } = usePulseChat({
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
    permissionLevel,
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
    setPulsePermissionLevel,
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

  // File selection effect — set documentMarkdown from markdownContent
  useEffect(() => {
    if (!selectedFile || !markdownContent) return
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

  // Keyboard shortcut effect — model/permission hotkeys
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
  }, [setPulseModel, setPulsePermissionLevel])

  // Workspace prompt handler effect
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

  return (
    <div className={`space-y-1.5 ${isDesktop ? 'mt-1' : 'pt-11'}`}>
      {/* Fixed mobile header — title + SRC + pane switcher */}
      {!isDesktop && chatHistory.length > 0 && (
        <div className="fixed left-0 right-0 top-0 z-[9] flex h-11 items-center gap-2 border-b border-[var(--border-subtle)] bg-[rgba(3,7,18,0.45)] pl-3 pr-28 backdrop-blur-lg lg:hidden">
          {/* Space for AXON logo (fixed left-6 top-5 z-10) */}
          <div className="w-14 shrink-0" />
          {/* Spacer */}
          <div className="flex-1" />
          {/* SRC button + pane switcher */}
          <div className="flex shrink-0 items-center gap-1.5">
            <button
              type="button"
              onClick={() => setSourcesExpanded((prev) => !prev)}
              className="ui-chip inline-flex items-center gap-1 rounded border border-[rgba(95,135,175,0.24)] bg-[rgba(10,18,35,0.45)] px-1.5 py-0.5 text-[var(--text-dim)]"
              aria-expanded={sourcesExpanded}
              title={sourcesExpanded ? 'Hide sources' : 'Show sources'}
            >
              <BookOpen className="size-3.5" />
              {Math.max(activeThreadSources.length, latestCitationCount)}
              <ChevronDown
                className={`size-3.5 transition-transform ${sourcesExpanded ? 'rotate-180' : ''}`}
              />
            </button>
            <PulseMobilePaneSwitcher mobilePane={mobilePane} onMobilePaneChange={setMobilePane} />
          </div>
        </div>
      )}

      {/* Desktop toolbar */}
      {isDesktop && (
        <PulseToolbar
          title={documentTitle}
          onTitleChange={setDocumentTitle}
          isDesktop={isDesktop}
          onNewSession={handleNewSession}
        />
      )}
      <div className="flex h-[calc(100dvh-9rem)] overflow-hidden rounded-xl bg-[rgba(10,18,35,0.42)] shadow-[var(--shadow-md)] lg:h-[calc(100vh-12rem)]">
        <div
          ref={splitContainerRef}
          className="flex h-full min-w-0 flex-1 flex-col gap-1.5 p-1.5 lg:flex-row lg:gap-0"
        >
          {/* ── Chat panel ── */}
          <div
            className={`group/chat relative flex h-full flex-col overflow-hidden rounded-xl bg-[rgba(10,18,35,0.52)] transition-all duration-200 ${
              isDesktop
                ? showChat
                  ? 'lg:flex-1'
                  : 'lg:w-7 lg:flex-none'
                : mobilePane === 'chat'
                  ? 'flex'
                  : 'hidden'
            }`}
          >
            {isDesktop && !showChat ? (
              /* Collapsed chat strip */
              <button
                type="button"
                onClick={() => toggleChat(true)}
                aria-label="Expand chat"
                title="Expand chat"
                className="flex h-full w-7 flex-col items-center justify-center text-[var(--text-dim)] transition-colors hover:text-[var(--axon-primary)]"
              >
                <ChevronRight className="size-4" />
              </button>
            ) : (
              <>
                <PulseChatPane
                  messages={chatHistory}
                  isLoading={isChatLoading}
                  streamingPhase={streamPhase}
                  liveToolUses={liveToolUses}
                  onCancelRequest={handleCancelPrompt}
                  indexedSources={indexedSources}
                  activeThreadSources={activeThreadSources}
                  onRemoveSource={(url) =>
                    setActiveThreadSources((prev) => prev.filter((u) => u !== url))
                  }
                  onRetry={(prompt) => void handlePrompt(prompt)}
                  sourcesExpanded={sourcesExpanded}
                  onSourcesExpandedChange={setSourcesExpanded}
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
                {/* Collapse chat button — right inner edge, desktop only */}
                {isDesktop && (
                  <button
                    type="button"
                    onClick={() => toggleChat(false)}
                    aria-label="Collapse chat"
                    title="Collapse chat"
                    className="absolute right-0 top-1/2 z-10 flex h-10 w-4 -translate-y-1/2 items-center justify-center rounded-l border border-r-0 border-[var(--border-subtle)] bg-[rgba(10,18,35,0.72)] text-[var(--text-dim)] opacity-0 transition-opacity hover:text-[var(--axon-primary)] group-hover/chat:opacity-100"
                  >
                    <ChevronLeft className="size-3" />
                  </button>
                )}
              </>
            )}
          </div>

          {/* ── Drag handle (desktop, both panels open) ── */}
          {isDesktop && (
            <div
              ref={splitHandleRef}
              role="separator"
              aria-label="Resize chat/editor — drag or click to toggle editor"
              aria-orientation="vertical"
              aria-valuenow={Math.round(desktopSplitPercent)}
              aria-valuemin={20}
              aria-valuemax={80}
              className={`group mx-0.5 hidden w-2 cursor-col-resize items-center justify-center rounded-sm transition-colors hover:bg-[var(--border-subtle)] ${
                showChat && showEditor ? 'lg:flex' : 'lg:hidden'
              }`}
              onPointerDown={(event) => {
                dragStartRef.current = {
                  pointerX: event.clientX,
                  startPercent: desktopSplitPercent,
                }
                splitHandleRef.current?.classList.add('bg-[rgba(175,215,255,0.15)]')
              }}
            >
              <div className="flex flex-col gap-0.5 opacity-30 transition-opacity group-hover:opacity-70">
                {[0, 1, 2, 3, 4].map((i) => (
                  <div key={i} className="size-0.5 rounded-full bg-[var(--text-muted)]" />
                ))}
              </div>
            </div>
          )}

          {/* ── Editor panel ── */}
          <div
            className={`group/editor relative flex h-full flex-col overflow-hidden rounded-xl bg-[rgba(10,18,35,0.5)] transition-all duration-200 ${
              isDesktop
                ? showEditor
                  ? 'lg:flex-none'
                  : 'lg:w-7 lg:flex-none'
                : mobilePane === 'editor'
                  ? 'flex'
                  : 'hidden'
            }`}
            style={
              isDesktop && showEditor ? { flexBasis: `${100 - desktopSplitPercent}%` } : undefined
            }
          >
            {isDesktop && !showEditor ? (
              /* Collapsed editor strip */
              <button
                type="button"
                onClick={() => toggleEditor(true)}
                aria-label="Expand editor"
                title="Expand editor"
                className="flex h-full w-7 flex-col items-center justify-center text-[var(--text-dim)] transition-colors hover:text-[var(--axon-primary)]"
              >
                <ChevronLeft className="size-4" />
              </button>
            ) : (
              <>
                {/* Collapse editor button — left inner edge, desktop only */}
                {isDesktop && (
                  <button
                    type="button"
                    onClick={() => toggleEditor(false)}
                    aria-label="Collapse editor"
                    title="Collapse editor"
                    className="absolute left-0 top-1/2 z-10 flex h-10 w-4 -translate-y-1/2 items-center justify-center rounded-r border border-l-0 border-[var(--border-subtle)] bg-[rgba(10,18,35,0.72)] text-[var(--text-dim)] opacity-0 transition-opacity hover:text-[var(--axon-primary)] group-hover/editor:opacity-100"
                  >
                    <ChevronRight className="size-3" />
                  </button>
                )}
                <PulseEditorPane
                  markdown={documentMarkdown}
                  onMarkdownChange={setDocumentMarkdown}
                  scrollStorageKey="axon.web.pulse.editor-scroll"
                />
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
