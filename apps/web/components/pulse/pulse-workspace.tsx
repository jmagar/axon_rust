'use client'

import { BookOpen, ChevronDown } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { CrawlFileExplorer } from '@/components/crawl-file-explorer'
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
    crawlFiles,
    selectedFile,
    selectFile,
    currentJobId,
    markdownContent,
  } = useWsMessages()
  const { subscribe } = useAxonWs()

  const [documentMarkdown, setDocumentMarkdown] = useState('')
  const [documentTitle, setDocumentTitle] = useState('Untitled')
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
    desktopViewMode,
    setDesktopViewMode,
    desktopPaneOrder,
    setDesktopPaneOrder,
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
    chatSessionId,
    indexedSources,
    activeThreadSources,
    desktopSplitPercent,
    mobileSplitPercent,
    lastResponseLatencyMs,
    lastResponseModel,
    desktopViewMode,
    desktopPaneOrder,
    setPulsePermissionLevel,
    setPulseModel,
    setDocumentMarkdown,
    setChatHistory,
    setDocumentTitle,
    setChatSessionId,
    setIndexedSources,
    setActiveThreadSources,
    setDesktopSplitPercent,
    setMobileSplitPercent,
    setLastResponseLatencyMs,
    setLastResponseModel,
    setDesktopViewMode,
    setDesktopPaneOrder,
    messageIdRef,
  })

  const { saveStatus } = usePulseAutosave(documentMarkdown, documentTitle)

  // File selection effect — set documentMarkdown from markdownContent
  useEffect(() => {
    if (!selectedFile || !markdownContent) return
    setDocumentMarkdown(markdownContent)
    const parts = selectedFile.split('/')
    setDocumentTitle(parts[parts.length - 1] ?? selectedFile)
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
        <div className="fixed left-0 right-0 top-0 z-[9] flex h-11 items-center gap-2 border-b border-[rgba(255,135,175,0.1)] bg-[rgba(3,7,18,0.45)] pl-3 pr-28 backdrop-blur-lg lg:hidden">
          {/* Space for AXON logo (fixed left-6 top-5 z-10) */}
          <div className="w-14 shrink-0" />
          {/* Spacer */}
          <div className="flex-1" />
          {/* SRC button + pane switcher */}
          <div className="flex shrink-0 items-center gap-1.5">
            <button
              type="button"
              onClick={() => setSourcesExpanded((prev) => !prev)}
              className="ui-chip inline-flex items-center gap-1 rounded border border-[rgba(95,135,175,0.24)] bg-[rgba(10,18,35,0.45)] px-1.5 py-0.5 text-[var(--axon-text-subtle)]"
              aria-expanded={sourcesExpanded}
              title={sourcesExpanded ? 'Hide sources' : 'Show sources'}
            >
              <BookOpen className="size-3" />
              {Math.max(activeThreadSources.length, latestCitationCount)}
              <ChevronDown
                className={`size-3 transition-transform ${sourcesExpanded ? 'rotate-180' : ''}`}
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
          desktopViewMode={desktopViewMode}
          onDesktopViewModeChange={setDesktopViewMode}
          desktopPaneOrder={desktopPaneOrder}
          onSwapPanes={() =>
            setDesktopPaneOrder((prev) => (prev === 'editor-first' ? 'chat-first' : 'editor-first'))
          }
          onNewSession={handleNewSession}
        />
      )}
      <div className="flex h-[calc(100dvh-9rem)] overflow-hidden rounded-xl border border-[rgba(255,135,175,0.1)] bg-[rgba(10,18,35,0.42)] lg:h-[calc(100vh-12rem)]">
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
            aria-valuenow={0}
            aria-orientation="vertical"
            className={`w-2 cursor-col-resize rounded bg-[rgba(255,135,175,0.14)] transition-colors hover:bg-[rgba(175,215,255,0.2)] ${desktopViewMode === 'both' ? 'hidden lg:block' : 'hidden'}`}
            style={{ order: isDesktop ? 2 : 2 }}
            onPointerDown={(event) => {
              dragStartRef.current = {
                pointerX: event.clientX,
                startPercent: desktopSplitPercent,
              }
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
            style={{
              order: isDesktop ? (desktopPaneOrder === 'editor-first' ? 3 : 1) : 1,
            }}
          >
            <PulseChatPane
              messages={chatHistory}
              isLoading={isChatLoading}
              streamingPhase={streamPhase}
              liveToolUses={liveToolUses}
              onCancelRequest={handleCancelPrompt}
              indexedSources={indexedSources}
              activeThreadSources={activeThreadSources}
              onRemoveSource={(url) =>
                setActiveThreadSources((prev) => prev.filter((existingUrl) => existingUrl !== url))
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
          </div>
        </div>
      </div>
    </div>
  )
}
