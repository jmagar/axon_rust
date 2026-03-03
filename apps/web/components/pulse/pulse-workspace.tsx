'use client'

import { BookOpen, ChevronDown, ChevronLeft, ChevronRight } from 'lucide-react'
import dynamic from 'next/dynamic'
import { usePulseWorkspaceBehavior } from '@/hooks/use-pulse-workspace'
import { PulseChatPane } from './pulse-chat-pane'
import { PulseMobilePaneSwitcher } from './pulse-mobile-pane-switcher'
import { PulseOpConfirmation } from './pulse-op-confirmation'
import { PulseToolbar } from './pulse-toolbar'

const PulseEditorPane = dynamic(
  () => import('./pulse-editor-pane').then((m) => ({ default: m.PulseEditorPane })),
  {
    ssr: false,
    loading: () => (
      <div className="flex h-full items-center justify-center text-[var(--text-dim)]">
        Loading editor…
      </div>
    ),
  },
)

export function PulseWorkspace() {
  const ws = usePulseWorkspaceBehavior()

  return (
    <div className={`flex h-full flex-col${!ws.isDesktop ? ' pt-11' : ''}`}>
      {/* Fixed mobile header — title + SRC + pane switcher */}
      {!ws.isDesktop && ws.chatHistory.length > 0 && (
        <div className="fixed left-0 right-0 top-0 z-[9] flex h-11 items-center gap-2 border-b border-[var(--border-subtle)] bg-[rgba(3,7,18,0.45)] pl-3 pr-28 backdrop-blur-lg lg:hidden">
          <div className="w-14 shrink-0" />
          <div className="flex-1" />
          <div className="flex shrink-0 items-center gap-1.5">
            <button
              type="button"
              onClick={() => ws.setSourcesExpanded((prev) => !prev)}
              className="ui-chip inline-flex items-center gap-1 rounded border border-[rgba(95,135,175,0.24)] bg-[rgba(10,18,35,0.45)] px-1.5 py-0.5 text-[var(--text-dim)]"
              aria-expanded={ws.sourcesExpanded}
              title={ws.sourcesExpanded ? 'Hide sources' : 'Show sources'}
            >
              <BookOpen className="size-3.5" />
              {Math.max(ws.activeThreadSources.length, ws.latestCitationCount) > 0 && (
                <span>{Math.max(ws.activeThreadSources.length, ws.latestCitationCount)}</span>
              )}
              <ChevronDown
                className={`size-3.5 transition-transform ${ws.sourcesExpanded ? 'rotate-180' : ''}`}
              />
            </button>
            <PulseMobilePaneSwitcher
              mobilePane={ws.mobilePane}
              onMobilePaneChange={ws.setMobilePane}
            />
          </div>
        </div>
      )}

      {/* Desktop toolbar */}
      {ws.isDesktop && (
        <PulseToolbar
          title={ws.documentTitle}
          onTitleChange={ws.setDocumentTitle}
          isDesktop={ws.isDesktop}
          onNewSession={ws.handleNewSession}
        />
      )}

      <div className="flex flex-1 overflow-hidden bg-[rgba(10,18,35,0.42)]">
        <div
          ref={ws.splitContainerRef}
          className="flex h-full min-w-0 flex-1 flex-col gap-1.5 p-1.5 lg:flex-row lg:gap-0"
        >
          {/* Chat panel */}
          <div
            className={`group/chat relative flex h-full flex-col overflow-hidden rounded-xl bg-[rgba(10,18,35,0.52)] transition-[flex-basis,width] duration-200 ease-[cubic-bezier(0.16,1,0.3,1)] ${
              ws.isDesktop
                ? ws.showChat
                  ? 'lg:flex-1'
                  : 'lg:w-7 lg:flex-none'
                : ws.mobilePane === 'chat'
                  ? 'flex'
                  : 'hidden'
            }`}
          >
            {ws.isDesktop && !ws.showChat ? (
              <button
                type="button"
                onClick={() => ws.toggleChat(true)}
                aria-label="Expand chat"
                title="Expand chat [⌘⇧C]"
                className="flex h-full w-7 lg:w-8 flex-col items-center justify-center gap-2 border-l border-[rgba(135,175,255,0.15)] text-[var(--text-dim)] transition-colors hover:text-[var(--axon-primary)]"
              >
                <ChevronRight className="size-4" />
                <span className="[writing-mode:vertical-rl] rotate-180 text-[length:var(--text-2xs)] tracking-widest uppercase">
                  CHAT
                </span>
              </button>
            ) : (
              <>
                <PulseChatPane
                  messages={ws.chatHistory}
                  isLoading={ws.isChatLoading}
                  streamingPhase={ws.streamPhase}
                  liveToolUses={ws.liveToolUses}
                  onCancelRequest={ws.handleCancelPrompt}
                  indexedSources={ws.indexedSources}
                  activeThreadSources={ws.activeThreadSources}
                  onRemoveSource={(url) =>
                    ws.setActiveThreadSources((prev) => prev.filter((u) => u !== url))
                  }
                  onRetry={(prompt) => void ws.handlePrompt(prompt)}
                  sourcesExpanded={ws.sourcesExpanded}
                  onSourcesExpandedChange={ws.setSourcesExpanded}
                  requestNotice={ws.requestNotice}
                />
                {ws.pendingOps && ws.pendingValidation && (
                  <PulseOpConfirmation
                    operations={ws.pendingOps}
                    validation={ws.pendingValidation}
                    onConfirm={() => {
                      ws.applyOperations(ws.pendingOps!)
                      ws.setPendingOps(null)
                      ws.setPendingValidation(null)
                    }}
                    onReject={() => {
                      ws.setPendingOps(null)
                      ws.setPendingValidation(null)
                    }}
                  />
                )}
                {ws.isDesktop && (
                  <button
                    type="button"
                    onClick={() => ws.toggleChat(false)}
                    aria-label="Collapse chat"
                    title="Collapse chat [⌘⇧C]"
                    className="absolute right-0 top-1/2 z-10 flex h-10 w-4 -translate-y-1/2 items-center justify-center rounded-l border border-r-0 border-[var(--border-subtle)] bg-[rgba(10,18,35,0.72)] text-[var(--text-dim)] opacity-0 transition-opacity hover:text-[var(--axon-primary)] group-hover/chat:opacity-100"
                  >
                    <ChevronLeft className="size-3" />
                  </button>
                )}
              </>
            )}
          </div>

          {/* Drag handle (desktop, both panels open) */}
          {ws.isDesktop && (
            <div
              ref={ws.splitHandleRef}
              role="separator"
              aria-label="Resize chat/editor — drag or click to toggle editor"
              title="Drag to resize · Click to toggle editor [⌘⇧E]"
              aria-orientation="vertical"
              aria-valuenow={Math.round(ws.desktopSplitPercent)}
              aria-valuemin={20}
              aria-valuemax={80}
              aria-valuetext={`Chat: ${Math.round(ws.desktopSplitPercent)}%, Editor: ${Math.round(100 - ws.desktopSplitPercent)}%`}
              className={`group mx-0.5 hidden w-2 cursor-col-resize items-center justify-center rounded-sm transition-colors hover:bg-[var(--border-subtle)] ${
                ws.showChat && ws.showEditor ? 'lg:flex' : 'lg:hidden'
              }`}
              onPointerDown={(event) => {
                ws.dragStartRef.current = {
                  pointerX: event.clientX,
                  startPercent: ws.desktopSplitPercent,
                }
                ws.splitHandleRef.current?.classList.add('bg-[rgba(175,215,255,0.15)]')
              }}
            >
              <div className="flex flex-col gap-1 opacity-30 transition-opacity group-hover:opacity-70">
                {[0, 1, 2, 3, 4].map((i) => (
                  <div key={i} className="size-0.5 rounded-full bg-[var(--text-muted)]" />
                ))}
              </div>
            </div>
          )}

          {/* Editor panel */}
          <div
            className={`group/editor relative flex h-full flex-col overflow-hidden rounded-xl bg-[rgba(10,18,35,0.5)] transition-[flex-basis,width] duration-200 ease-[cubic-bezier(0.16,1,0.3,1)] ${
              ws.isDesktop
                ? ws.showEditor
                  ? ws.showChat
                    ? 'lg:flex-none'
                    : 'lg:flex-1'
                  : 'lg:w-7 lg:flex-none'
                : ws.mobilePane === 'editor'
                  ? 'flex'
                  : 'hidden'
            }`}
            style={
              ws.isDesktop && ws.showEditor && ws.showChat
                ? { flexBasis: `${100 - ws.desktopSplitPercent}%` }
                : undefined
            }
          >
            {ws.isDesktop && !ws.showEditor ? (
              <button
                type="button"
                onClick={() => ws.toggleEditor(true)}
                aria-label="Expand editor"
                title="Expand editor [⌘⇧E]"
                className="flex h-full w-7 lg:w-8 flex-col items-center justify-center gap-2 border-r border-[rgba(135,175,255,0.15)] text-[var(--text-dim)] transition-colors hover:text-[var(--axon-primary)]"
              >
                <ChevronLeft className="size-4" />
                <span className="[writing-mode:vertical-rl] rotate-180 text-[length:var(--text-2xs)] tracking-widest uppercase">
                  EDIT
                </span>
              </button>
            ) : (
              <>
                {ws.isDesktop && (
                  <button
                    type="button"
                    onClick={() => ws.toggleEditor(false)}
                    aria-label="Collapse editor"
                    title="Collapse editor [⌘⇧E]"
                    className="absolute left-0 top-1/2 z-10 flex h-10 w-4 -translate-y-1/2 items-center justify-center rounded-r border border-l-0 border-[var(--border-subtle)] bg-[rgba(10,18,35,0.72)] text-[var(--text-dim)] opacity-0 transition-opacity hover:text-[var(--axon-primary)] group-hover/editor:opacity-100"
                  >
                    <ChevronRight className="size-3" />
                  </button>
                )}
                <PulseEditorPane
                  markdown={ws.documentMarkdown}
                  onMarkdownChange={ws.setDocumentMarkdown}
                  scrollStorageKey={
                    ws.currentDocFilename
                      ? `axon.web.pulse.editor-scroll.${ws.currentDocFilename}`
                      : 'axon.web.pulse.editor-scroll'
                  }
                />
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
