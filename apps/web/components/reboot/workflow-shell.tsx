'use client'

import {
  Command,
  FileCode2,
  LayoutPanelLeft,
  LayoutPanelTop,
  MessageSquareText,
  Minimize2,
  PanelLeftClose,
  PanelRightClose,
  Play,
  Sparkles,
  TerminalSquare,
} from 'lucide-react'
import Link from 'next/link'
import { useEffect, useMemo, useState } from 'react'
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from '@/components/ai-elements/conversation'
import {
  Message,
  MessageAction,
  MessageActions,
  MessageContent,
  MessageResponse,
} from '@/components/ai-elements/message'
import {
  Queue,
  QueueItem,
  QueueItemContent,
  QueueItemDescription,
  QueueItemIndicator,
  QueueList,
  QueueSection,
  QueueSectionContent,
  QueueSectionLabel,
  QueueSectionTrigger,
} from '@/components/ai-elements/queue'
import { Reasoning, ReasoningContent, ReasoningTrigger } from '@/components/ai-elements/reasoning'
import {
  EDITOR_CONTENT,
  LOG_LINES,
  TERMINAL_LINES,
  WORKFLOW_CONVERSATIONS,
  WORKFLOW_SESSIONS,
} from './data'
import { RebootFrame } from './reboot-frame'

type PaneState = {
  sidebar: boolean
  chat: boolean
  editor: boolean
}

export function WorkflowShell() {
  const [panes, setPanes] = useState<PaneState>({ sidebar: true, chat: true, editor: false })
  const [sidebarWidth, setSidebarWidth] = useState(270)
  const [editorWidth, setEditorWidth] = useState(430)
  const [terminalOpen, setTerminalOpen] = useState(false)
  const [selectedSessionId, setSelectedSessionId] = useState(WORKFLOW_SESSIONS[0]!.id)
  const [prompt, setPrompt] = useState(
    'Seed the lobe, resume the active session, and open the PRD file when the plan is stable.',
  )
  const [activeFile, setActiveFile] = useState('apps/web/components/reboot/workflow-shell.tsx')

  const selectedSession = useMemo(
    () =>
      WORKFLOW_SESSIONS.find((session) => session.id === selectedSessionId) ??
      WORKFLOW_SESSIONS[0]!,
    [selectedSessionId],
  )
  const messages = WORKFLOW_CONVERSATIONS[selectedSession.id] ?? []

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || !event.shiftKey || event.key.toLowerCase() !== 'e') {
        return
      }
      event.preventDefault()
      setPanes((current) => ({ ...current, editor: true }))
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  const gridTemplateColumns = useMemo(() => {
    const widths: string[] = []
    if (panes.sidebar) widths.push(`minmax(${sidebarWidth}px, ${sidebarWidth}px)`)
    if (panes.chat) widths.push('minmax(0, 1fr)')
    if (panes.editor) widths.push(`minmax(${editorWidth}px, ${editorWidth}px)`)
    return widths.length ? widths.join(' ') : 'minmax(0, 1fr)'
  }, [editorWidth, panes.chat, panes.editor, panes.sidebar, sidebarWidth])

  const visiblePaneCount = Number(panes.sidebar) + Number(panes.chat) + Number(panes.editor)

  const openFile = (path: string) => {
    setActiveFile(path)
    setPanes((current) => ({ ...current, editor: true }))
  }

  return (
    <RebootFrame>
      <div className="mx-auto max-w-[1480px] px-4 pb-6 pt-5 md:px-6 xl:px-8">
        <header className="mb-5 flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
          <div>
            <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
              <Sparkles className="size-4 text-[var(--axon-primary)]" />
              <span>Workflow shell</span>
            </div>
            <h1 className="mt-3 text-[clamp(2.2rem,4vw,4.6rem)] font-semibold tracking-[-0.05em] text-[var(--text-primary)]">
              Sessions on the left. Conversation in the center. Editor on demand.
            </h1>
            <p className="mt-3 max-w-3xl text-sm leading-7 text-[var(--text-secondary)] md:text-base">
              This shell is global across projects. It keeps recent sessions small, the active chat
              central, and the editor contextual. Use{' '}
              <span className="font-semibold text-[var(--text-primary)]">Ctrl+Shift+E</span> or{' '}
              <span className="font-semibold text-[var(--text-primary)]">Cmd+Shift+E</span> to
              force-open the editor.
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Link
              className="rounded-full border border-[rgba(135,175,255,0.16)] bg-[rgba(7,12,26,0.72)] px-4 py-2 text-sm text-[var(--text-secondary)]"
              href="/reboot"
            >
              Shell chooser
            </Link>
            <Link
              className="rounded-full border border-[rgba(135,175,255,0.16)] bg-[rgba(7,12,26,0.72)] px-4 py-2 text-sm text-[var(--text-secondary)]"
              href="/reboot/lobe"
            >
              Open lobe
            </Link>
          </div>
        </header>

        <section className="mb-4 rounded-[26px] border border-[rgba(135,175,255,0.14)] bg-[rgba(7,12,26,0.76)] p-4 shadow-[0_18px_50px_rgba(0,0,0,0.24)]">
          <div className="flex flex-wrap items-center gap-2">
            {[
              ['sidebar', 'Sessions'],
              ['chat', 'Chat'],
              ['editor', 'Editor'],
            ].map(([key, label]) => (
              <button
                key={key}
                className={`rounded-full px-3 py-1.5 text-xs ${
                  panes[key as keyof PaneState]
                    ? 'border border-[rgba(255,135,175,0.22)] bg-[rgba(255,135,175,0.12)] text-[var(--text-primary)]'
                    : 'border border-[rgba(135,175,255,0.12)] text-[var(--text-muted)]'
                }`}
                onClick={() =>
                  setPanes((current) => ({
                    ...current,
                    [key]: !current[key as keyof PaneState],
                  }))
                }
                type="button"
              >
                {label}
              </button>
            ))}

            <button
              className="rounded-full border border-[rgba(135,175,255,0.12)] px-3 py-1.5 text-xs text-[var(--text-muted)]"
              onClick={() => setTerminalOpen((current) => !current)}
              type="button"
            >
              Terminal drawer
            </button>

            <button
              className="rounded-full border border-[rgba(135,175,255,0.12)] px-3 py-1.5 text-xs text-[var(--text-muted)]"
              onClick={() => setPanes({ sidebar: true, chat: true, editor: false })}
              type="button"
            >
              Sidebar + chat
            </button>
            <button
              className="rounded-full border border-[rgba(135,175,255,0.12)] px-3 py-1.5 text-xs text-[var(--text-muted)]"
              onClick={() => setPanes({ sidebar: true, chat: true, editor: true })}
              type="button"
            >
              Full workspace
            </button>
            <button
              className="rounded-full border border-[rgba(135,175,255,0.12)] px-3 py-1.5 text-xs text-[var(--text-muted)]"
              onClick={() => setPanes({ sidebar: false, chat: true, editor: false })}
              type="button"
            >
              Chat only
            </button>
            <button
              className="rounded-full border border-[rgba(135,175,255,0.12)] px-3 py-1.5 text-xs text-[var(--text-muted)]"
              onClick={() => setPanes({ sidebar: false, chat: false, editor: true })}
              type="button"
            >
              Editor only
            </button>
          </div>

          <div className="mt-4 grid gap-3 md:grid-cols-2">
            <label className="rounded-2xl border border-[rgba(135,175,255,0.12)] bg-[rgba(255,255,255,0.03)] px-4 py-3 text-xs text-[var(--text-muted)]">
              Session sidebar width
              <input
                className="mt-3 w-full accent-[var(--axon-primary)]"
                max={360}
                min={220}
                onChange={(event) => setSidebarWidth(Number(event.target.value))}
                type="range"
                value={sidebarWidth}
              />
            </label>
            <label className="rounded-2xl border border-[rgba(135,175,255,0.12)] bg-[rgba(255,255,255,0.03)] px-4 py-3 text-xs text-[var(--text-muted)]">
              Editor width
              <input
                className="mt-3 w-full accent-[var(--axon-secondary)]"
                max={620}
                min={340}
                onChange={(event) => setEditorWidth(Number(event.target.value))}
                type="range"
                value={editorWidth}
              />
            </label>
          </div>
        </section>

        <section
          className="grid min-h-[calc(100vh-15rem)] gap-3 transition-[grid-template-columns] duration-500 ease-[cubic-bezier(0.22,1,0.36,1)]"
          style={{ gridTemplateColumns }}
        >
          {panes.sidebar ? (
            <aside className="min-h-0 rounded-[30px] border border-[rgba(135,175,255,0.14)] bg-[rgba(6,12,26,0.82)] p-4 shadow-[0_18px_60px_rgba(0,0,0,0.28)]">
              <div className="flex items-center justify-between gap-3">
                <div>
                  <p className="text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
                    Global sessions
                  </p>
                  <h2 className="mt-2 text-sm font-semibold text-[var(--text-primary)]">
                    Across all repos
                  </h2>
                </div>
                <button
                  className="inline-flex size-8 items-center justify-center rounded-full border border-[rgba(135,175,255,0.14)] bg-[rgba(10,18,35,0.55)] text-[var(--text-muted)]"
                  onClick={() => setPanes((current) => ({ ...current, sidebar: false }))}
                  type="button"
                >
                  <PanelLeftClose className="size-4" />
                </button>
              </div>

              <div className="mt-4 space-y-2">
                {WORKFLOW_SESSIONS.map((session) => (
                  <button
                    key={session.id}
                    className={`w-full rounded-[22px] border p-3 text-left transition-colors ${
                      session.id === selectedSession.id
                        ? 'border-[rgba(255,135,175,0.24)] bg-[rgba(255,135,175,0.12)]'
                        : 'border-[rgba(135,175,255,0.12)] bg-[rgba(255,255,255,0.03)]'
                    }`}
                    onClick={() => setSelectedSessionId(session.id)}
                    type="button"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <span className="text-xs uppercase tracking-[0.2em] text-[var(--text-dim)]">
                        {session.agent}
                      </span>
                      <span className="text-[10px] uppercase tracking-[0.2em] text-[var(--text-muted)]">
                        {session.status}
                      </span>
                    </div>
                    <p className="mt-2 text-sm font-medium text-[var(--text-primary)]">
                      {session.title}
                    </p>
                    <p className="mt-2 text-xs text-[var(--text-secondary)]">
                      {session.repo} · {session.branch}
                    </p>
                  </button>
                ))}
              </div>
            </aside>
          ) : null}

          {panes.chat ? (
            <section className="flex min-h-0 flex-col overflow-hidden rounded-[30px] border border-[rgba(135,175,255,0.14)] bg-[linear-gradient(180deg,rgba(10,18,35,0.92),rgba(4,8,20,0.82))] shadow-[0_18px_80px_rgba(0,0,0,0.32)]">
              <div className="flex items-center justify-between gap-4 border-b border-[rgba(135,175,255,0.1)] px-5 py-4">
                <div>
                  <p className="text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
                    Conversation
                  </p>
                  <h2 className="mt-2 text-sm font-semibold text-[var(--text-primary)]">
                    {selectedSession.title}
                  </h2>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    className="inline-flex size-8 items-center justify-center rounded-full border border-[rgba(135,175,255,0.14)] bg-[rgba(10,18,35,0.55)] text-[var(--text-muted)]"
                    onClick={() =>
                      setPanes((current) => ({ ...current, chat: visiblePaneCount === 1 }))
                    }
                    type="button"
                  >
                    <Minimize2 className="size-4" />
                  </button>
                  {!panes.sidebar ? (
                    <button
                      className="inline-flex size-8 items-center justify-center rounded-full border border-[rgba(135,175,255,0.14)] bg-[rgba(10,18,35,0.55)] text-[var(--text-muted)]"
                      onClick={() => setPanes((current) => ({ ...current, sidebar: true }))}
                      type="button"
                    >
                      <LayoutPanelLeft className="size-4" />
                    </button>
                  ) : null}
                  {!panes.editor ? (
                    <button
                      className="inline-flex size-8 items-center justify-center rounded-full border border-[rgba(255,135,175,0.18)] bg-[rgba(255,135,175,0.08)] text-[var(--text-muted)]"
                      onClick={() => setPanes((current) => ({ ...current, editor: true }))}
                      type="button"
                    >
                      <FileCode2 className="size-4" />
                    </button>
                  ) : null}
                </div>
              </div>

              <div className="border-b border-[rgba(135,175,255,0.1)] px-5 py-4">
                <div className="rounded-[26px] border border-[rgba(255,135,175,0.16)] bg-[rgba(255,135,175,0.08)] p-4">
                  <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
                    <Command className="size-4 text-[var(--axon-primary)]" />
                    <span>Workflow omnibox</span>
                  </div>
                  <textarea
                    className="mt-4 h-24 w-full resize-none rounded-[22px] border border-[rgba(135,175,255,0.16)] bg-[rgba(4,8,20,0.75)] px-4 py-3 text-sm leading-7 text-[var(--text-primary)] outline-none"
                    onChange={(event) => setPrompt(event.target.value)}
                    value={prompt}
                  />
                  <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
                    <div className="flex flex-wrap gap-2">
                      {['repo context', 'docs', 'session memory', 'PRD', 'logs'].map((chip) => (
                        <span
                          key={chip}
                          className="rounded-full border border-[rgba(135,175,255,0.12)] px-3 py-1.5 text-xs text-[var(--text-muted)]"
                        >
                          {chip}
                        </span>
                      ))}
                    </div>
                    <button
                      className="inline-flex items-center gap-2 rounded-2xl border border-[rgba(255,135,175,0.22)] bg-[rgba(255,135,175,0.12)] px-4 py-2 text-sm text-[var(--text-primary)]"
                      type="button"
                    >
                      <Play className="size-4" />
                      Run session
                    </button>
                  </div>
                </div>
              </div>

              <div className="grid min-h-0 flex-1 gap-0 xl:grid-cols-[minmax(0,1fr)_18rem]">
                <Conversation className="min-h-0">
                  {messages.length ? (
                    <>
                      <ConversationContent className="pb-20">
                        {messages.map((message) => (
                          <Message from={message.role} key={message.id}>
                            <MessageContent>
                              <MessageResponse>{message.content}</MessageResponse>
                              {message.reasoning ? (
                                <Reasoning defaultOpen={false}>
                                  <ReasoningTrigger />
                                  <ReasoningContent>{message.reasoning}</ReasoningContent>
                                </Reasoning>
                              ) : null}
                              {message.files?.length ? (
                                <div className="mt-2 flex flex-wrap gap-2">
                                  {message.files.map((path) => (
                                    <button
                                      key={path}
                                      className="rounded-full border border-[rgba(135,175,255,0.14)] bg-[rgba(255,255,255,0.04)] px-3 py-1.5 text-xs text-[var(--text-secondary)]"
                                      onClick={() => openFile(path)}
                                      type="button"
                                    >
                                      {path}
                                    </button>
                                  ))}
                                </div>
                              ) : null}
                            </MessageContent>
                            {message.role === 'assistant' ? (
                              <MessageActions>
                                <MessageAction
                                  label="Open editor"
                                  onClick={() =>
                                    setPanes((current) => ({ ...current, editor: true }))
                                  }
                                  tooltip="Open editor"
                                >
                                  <FileCode2 className="size-4" />
                                </MessageAction>
                                <MessageAction
                                  label="Toggle terminal"
                                  onClick={() => setTerminalOpen((current) => !current)}
                                  tooltip="Toggle terminal"
                                >
                                  <TerminalSquare className="size-4" />
                                </MessageAction>
                              </MessageActions>
                            ) : null}
                          </Message>
                        ))}
                      </ConversationContent>
                      <ConversationScrollButton />
                    </>
                  ) : (
                    <ConversationEmptyState
                      description="Pick a session from the left rail or start from the omnibox."
                      icon={<MessageSquareText className="size-10" />}
                      title="No active conversation"
                    />
                  )}
                </Conversation>

                <div className="hidden min-h-0 border-l border-[rgba(135,175,255,0.1)] p-4 xl:block">
                  <Queue>
                    <QueueSection defaultOpen>
                      <QueueSectionTrigger>
                        <QueueSectionLabel
                          label="active flow"
                          icon={<LayoutPanelTop className="size-4" />}
                        />
                      </QueueSectionTrigger>
                      <QueueSectionContent>
                        <QueueList>
                          {[
                            {
                              title: 'Seed docs',
                              description: 'nextjs.org, platejs.org, agentclientprotocol.com',
                              done: true,
                            },
                            {
                              title: 'Resume session',
                              description: `${selectedSession.agent} · ${selectedSession.repo}`,
                              done: true,
                            },
                            { title: 'Draft PRD', description: activeFile, done: panes.editor },
                            {
                              title: 'Open terminal',
                              description: 'Quick command access from drawer',
                              done: terminalOpen,
                            },
                          ].map(({ title, description, done }) => (
                            <QueueItem key={title}>
                              <div className="flex items-start gap-3">
                                <QueueItemIndicator completed={Boolean(done)} />
                                <div className="min-w-0">
                                  <QueueItemContent completed={Boolean(done)}>
                                    {title}
                                  </QueueItemContent>
                                  <QueueItemDescription completed={Boolean(done)}>
                                    {description}
                                  </QueueItemDescription>
                                </div>
                              </div>
                            </QueueItem>
                          ))}
                        </QueueList>
                      </QueueSectionContent>
                    </QueueSection>
                  </Queue>
                </div>
              </div>
            </section>
          ) : null}

          {panes.editor ? (
            <aside className="min-h-0 rounded-[30px] border border-[rgba(135,175,255,0.14)] bg-[rgba(6,12,26,0.82)] shadow-[0_18px_60px_rgba(0,0,0,0.28)]">
              <div className="flex items-center justify-between gap-3 border-b border-[rgba(135,175,255,0.1)] px-5 py-4">
                <div>
                  <p className="text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
                    Editor
                  </p>
                  <h2 className="mt-2 text-sm font-semibold text-[var(--text-primary)]">
                    {activeFile}
                  </h2>
                </div>
                <button
                  className="inline-flex size-8 items-center justify-center rounded-full border border-[rgba(135,175,255,0.14)] bg-[rgba(10,18,35,0.55)] text-[var(--text-muted)]"
                  onClick={() => setPanes((current) => ({ ...current, editor: false }))}
                  type="button"
                >
                  <PanelRightClose className="size-4" />
                </button>
              </div>

              <div className="p-5">
                <div className="mb-4 flex flex-wrap gap-2">
                  {Object.keys(EDITOR_CONTENT)
                    .slice(0, 4)
                    .map((path) => (
                      <button
                        key={path}
                        className={`rounded-full px-3 py-1.5 text-xs ${
                          activeFile === path
                            ? 'border border-[rgba(255,135,175,0.22)] bg-[rgba(255,135,175,0.12)] text-[var(--text-primary)]'
                            : 'border border-[rgba(135,175,255,0.12)] text-[var(--text-muted)]'
                        }`}
                        onClick={() => setActiveFile(path)}
                        type="button"
                      >
                        {path.split('/').slice(-1)[0]}
                      </button>
                    ))}
                </div>

                <div className="min-h-[34rem] rounded-[24px] border border-[rgba(135,175,255,0.12)] bg-[rgba(4,8,20,0.72)] p-5">
                  <pre className="whitespace-pre-wrap font-mono text-sm leading-7 text-[var(--text-secondary)]">
                    {EDITOR_CONTENT[activeFile] ??
                      'Open a file from the conversation to populate the editor.'}
                  </pre>
                </div>
              </div>
            </aside>
          ) : null}
        </section>

        {terminalOpen ? (
          <section className="mt-3 rounded-[30px] border border-[rgba(135,175,255,0.14)] bg-[rgba(4,8,20,0.92)] p-5 shadow-[0_18px_50px_rgba(0,0,0,0.24)]">
            <div className="flex items-center justify-between gap-3">
              <div>
                <p className="text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
                  Terminal drawer
                </p>
                <h2 className="mt-2 text-sm font-semibold text-[var(--text-primary)]">
                  {selectedSession.repo}
                </h2>
              </div>
              <button
                className="inline-flex size-8 items-center justify-center rounded-full border border-[rgba(135,175,255,0.14)] bg-[rgba(10,18,35,0.55)] text-[var(--text-muted)]"
                onClick={() => setTerminalOpen(false)}
                type="button"
              >
                <PanelRightClose className="size-4 rotate-90" />
              </button>
            </div>

            <div className="mt-4 grid gap-3 xl:grid-cols-[minmax(0,1fr)_20rem]">
              <div className="rounded-[24px] border border-[rgba(135,175,255,0.12)] bg-[rgba(0,0,0,0.34)] p-4 font-mono text-sm leading-7 text-[var(--text-secondary)]">
                {TERMINAL_LINES.map((line) => (
                  <div key={line}>{line}</div>
                ))}
              </div>
              <div className="rounded-[24px] border border-[rgba(135,175,255,0.12)] bg-[rgba(255,255,255,0.03)] p-4">
                <p className="text-[11px] uppercase tracking-[0.3em] text-[var(--text-dim)]">
                  Logs
                </p>
                <div className="mt-3 space-y-2 text-sm text-[var(--text-secondary)]">
                  {LOG_LINES.map((line) => (
                    <div
                      key={line}
                      className="rounded-2xl border border-[rgba(135,175,255,0.08)] px-3 py-2"
                    >
                      {line}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </section>
        ) : null}
      </div>
    </RebootFrame>
  )
}
