'use client'

import {
  BookOpenText,
  Bot,
  Boxes,
  FolderTree,
  GitBranch,
  LayoutDashboard,
  Logs,
  Play,
  ScrollText,
  Search,
  Settings2,
  Sparkles,
  TerminalSquare,
} from 'lucide-react'
import Link from 'next/link'
import { useState } from 'react'
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
  CONFIG_LIBRARY,
  DOC_LIBRARY,
  LOBE_INDEXING,
  LOBE_METRICS,
  LOBE_ROADMAP,
  LOBE_TODOS,
  LOBES,
  OPS_SURFACES,
  REPO_TREE,
} from './data'
import { RebootFrame } from './reboot-frame'

type LobeSurface = 'overview' | 'knowledge' | 'sessions' | 'ops'
type ExplorerMode = 'repo' | 'docs' | 'config'

const SURFACES: Array<{ value: LobeSurface; label: string; icon: typeof LayoutDashboard }> = [
  { value: 'overview', label: 'Overview', icon: LayoutDashboard },
  { value: 'knowledge', label: 'Knowledge', icon: BookOpenText },
  { value: 'sessions', label: 'Sessions', icon: Bot },
  { value: 'ops', label: 'Ops', icon: TerminalSquare },
]

export function LobeShell() {
  const [surface, setSurface] = useState<LobeSurface>('overview')
  const [explorerMode, setExplorerMode] = useState<ExplorerMode>('repo')
  const [prompt, setPrompt] = useState(
    'Seed this repo into a lobe, ingest all agent sessions, group relevant Cortex docs by stack, and draft the execution roadmap.',
  )
  const activeLobe = LOBES[0]

  return (
    <RebootFrame>
      <div className="mx-auto max-w-[1360px] px-4 pb-10 pt-6 md:px-6 xl:px-8">
        <header className="mb-6 flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
          <div>
            <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
              <Sparkles className="size-4 text-[var(--axon-primary)]" />
              <span>{activeLobe.name} lobe</span>
            </div>
            <h1 className="mt-3 text-[clamp(2.2rem,4vw,4.6rem)] font-semibold tracking-[-0.05em] text-[var(--text-primary)]">
              Project home base for research, memory, and execution.
            </h1>
            <p className="mt-3 max-w-3xl text-sm leading-7 text-[var(--text-secondary)] md:text-base">
              The lobe is not the active work cockpit. It is the project dashboard, memory spine,
              and launch pad into docs, editor, logs, terminal, jobs, skills, MCP, and workflow.
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
              className="inline-flex items-center gap-2 rounded-full border border-[rgba(255,135,175,0.2)] bg-[rgba(255,135,175,0.1)] px-4 py-2 text-sm text-[var(--text-primary)]"
              href="/reboot/workflow"
            >
              <Play className="size-4" />
              Launch workflow
            </Link>
          </div>
        </header>

        <section className="grid gap-4 xl:grid-cols-[18rem_minmax(0,1fr)]">
          <aside className="rounded-[30px] border border-[rgba(135,175,255,0.14)] bg-[rgba(6,12,26,0.82)] p-4 shadow-[0_18px_60px_rgba(0,0,0,0.28)]">
            <div className="rounded-[24px] border border-[rgba(255,135,175,0.16)] bg-[rgba(255,135,175,0.08)] p-4">
              <p className="text-[11px] uppercase tracking-[0.3em] text-[var(--text-dim)]">
                Identity
              </p>
              <h2 className="mt-2 text-lg font-semibold text-[var(--text-primary)]">
                {activeLobe.name}
              </h2>
              <p className="mt-2 text-sm leading-6 text-[var(--text-secondary)]">
                {activeLobe.repo}
              </p>
              <div className="mt-4 space-y-2 text-xs text-[var(--text-muted)]">
                <div>{activeLobe.collection}</div>
                <div>{activeLobe.branch}</div>
                <div>
                  {activeLobe.sessions} sessions · {activeLobe.docs} docs
                </div>
              </div>
            </div>

            <div className="mt-4 space-y-2">
              {SURFACES.map(({ value, label, icon: Icon }) => (
                <button
                  key={value}
                  className={`flex w-full items-center gap-3 rounded-2xl px-4 py-3 text-left text-sm transition-colors ${
                    surface === value
                      ? 'border border-[rgba(175,215,255,0.2)] bg-[rgba(135,175,255,0.16)] text-[var(--text-primary)]'
                      : 'border border-transparent bg-[rgba(255,255,255,0.03)] text-[var(--text-secondary)] hover:border-[rgba(135,175,255,0.14)]'
                  }`}
                  onClick={() => setSurface(value)}
                  type="button"
                >
                  <Icon className="size-4" />
                  {label}
                </button>
              ))}
            </div>
          </aside>

          <div className="space-y-4">
            <section className="rounded-[30px] border border-[rgba(135,175,255,0.14)] bg-[linear-gradient(180deg,rgba(10,18,35,0.92),rgba(4,8,20,0.82))] p-5 shadow-[0_18px_80px_rgba(0,0,0,0.32)]">
              <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
                <Search className="size-4 text-[var(--axon-primary)]" />
                <span>Lobe omnibox</span>
              </div>
              <textarea
                className="mt-4 h-32 w-full resize-none rounded-[26px] border border-[rgba(135,175,255,0.16)] bg-[rgba(4,8,20,0.72)] px-5 py-4 text-base leading-7 text-[var(--text-primary)] outline-none"
                onChange={(event) => setPrompt(event.target.value)}
                value={prompt}
              />
              <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
                <div className="flex flex-wrap gap-2">
                  {['Repo', 'Sessions', 'Cortex docs', 'Issues', 'PRs', 'Logs', 'MCP'].map(
                    (chip) => (
                      <span
                        key={chip}
                        className="rounded-full border border-[rgba(135,175,255,0.12)] px-3 py-1.5 text-xs text-[var(--text-muted)]"
                      >
                        {chip}
                      </span>
                    ),
                  )}
                </div>
                <Link
                  className="inline-flex items-center gap-2 rounded-2xl border border-[rgba(255,135,175,0.22)] bg-[rgba(255,135,175,0.12)] px-4 py-2 text-sm text-[var(--text-primary)]"
                  href="/reboot/workflow"
                >
                  <Play className="size-4" />
                  Launch into workflow
                </Link>
              </div>
            </section>

            {surface === 'overview' ? (
              <>
                <section className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                  {LOBE_METRICS.map(([label, value]) => (
                    <div
                      key={label}
                      className="rounded-[26px] border border-[rgba(135,175,255,0.12)] bg-[rgba(7,12,26,0.72)] p-4"
                    >
                      <p className="text-[11px] uppercase tracking-[0.28em] text-[var(--text-dim)]">
                        {label}
                      </p>
                      <p className="mt-3 text-sm leading-6 text-[var(--text-primary)]">{value}</p>
                    </div>
                  ))}
                </section>

                <section className="grid gap-4 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
                  <Queue className="rounded-[30px]">
                    <QueueSection defaultOpen>
                      <QueueSectionTrigger>
                        <QueueSectionLabel
                          count={LOBE_INDEXING.length}
                          icon={<Boxes className="size-4" />}
                          label="indexing feeds"
                        />
                      </QueueSectionTrigger>
                      <QueueSectionContent>
                        <QueueList>
                          {LOBE_INDEXING.map(([title, description, status]) => (
                            <QueueItem key={title}>
                              <div className="flex items-start gap-3">
                                <QueueItemIndicator completed={status === 'completed'} />
                                <div className="min-w-0">
                                  <QueueItemContent completed={status === 'completed'}>
                                    {title}
                                  </QueueItemContent>
                                  <QueueItemDescription completed={status === 'completed'}>
                                    {description}
                                  </QueueItemDescription>
                                </div>
                              </div>
                            </QueueItem>
                          ))}
                        </QueueList>
                      </QueueSectionContent>
                    </QueueSection>

                    <QueueSection defaultOpen>
                      <QueueSectionTrigger>
                        <QueueSectionLabel
                          count={LOBE_TODOS.length}
                          icon={<ScrollText className="size-4" />}
                          label="project todos"
                        />
                      </QueueSectionTrigger>
                      <QueueSectionContent>
                        <QueueList>
                          {LOBE_TODOS.map((todo) => (
                            <QueueItem key={todo}>
                              <div className="flex items-start gap-3">
                                <QueueItemIndicator />
                                <QueueItemContent>{todo}</QueueItemContent>
                              </div>
                            </QueueItem>
                          ))}
                        </QueueList>
                      </QueueSectionContent>
                    </QueueSection>
                  </Queue>

                  <div className="space-y-4">
                    <Reasoning defaultOpen className="rounded-[30px]">
                      <ReasoningTrigger />
                      <ReasoningContent>
                        The lobe should summarize the project at a glance, but never try to become
                        the active cockpit. Its job is to keep repo memory, docs, session history,
                        and ops surfaces organized so the Workflow shell can stay fast and focused.
                      </ReasoningContent>
                    </Reasoning>

                    <div className="grid gap-3 md:grid-cols-3">
                      {LOBE_ROADMAP.map((item, index) => (
                        <div
                          key={item}
                          className="rounded-[24px] border border-[rgba(135,175,255,0.12)] bg-[rgba(7,12,26,0.72)] p-4"
                        >
                          <p className="text-[11px] uppercase tracking-[0.28em] text-[var(--text-dim)]">
                            Phase {index + 1}
                          </p>
                          <p className="mt-3 text-sm leading-7 text-[var(--text-secondary)]">
                            {item}
                          </p>
                        </div>
                      ))}
                    </div>
                  </div>
                </section>
              </>
            ) : null}

            {surface === 'knowledge' ? (
              <section className="rounded-[30px] border border-[rgba(135,175,255,0.14)] bg-[rgba(7,12,26,0.78)] p-5 shadow-[0_18px_60px_rgba(0,0,0,0.28)]">
                <div className="flex flex-wrap gap-2">
                  {(
                    [
                      ['repo', 'Repo tree'],
                      ['docs', 'Docs'],
                      ['config', 'Config'],
                    ] as const
                  ).map(([value, label]) => (
                    <button
                      key={value}
                      className={`rounded-full px-3 py-1.5 text-xs ${
                        explorerMode === value
                          ? 'border border-[rgba(255,135,175,0.22)] bg-[rgba(255,135,175,0.12)] text-[var(--text-primary)]'
                          : 'border border-[rgba(135,175,255,0.12)] text-[var(--text-muted)]'
                      }`}
                      onClick={() => setExplorerMode(value)}
                      type="button"
                    >
                      {label}
                    </button>
                  ))}
                </div>

                <div className="mt-4 grid gap-3 lg:grid-cols-2">
                  {(explorerMode === 'repo'
                    ? REPO_TREE
                    : explorerMode === 'config'
                      ? CONFIG_LIBRARY
                      : DOC_LIBRARY
                  ).map((entry) =>
                    typeof entry === 'string' ? (
                      <div
                        key={entry}
                        className="rounded-[22px] border border-[rgba(135,175,255,0.12)] bg-[rgba(255,255,255,0.03)] px-4 py-3 text-sm text-[var(--text-secondary)]"
                      >
                        {entry}
                      </div>
                    ) : (
                      <div
                        key={entry[0]}
                        className="rounded-[22px] border border-[rgba(135,175,255,0.12)] bg-[rgba(255,255,255,0.03)] p-4"
                      >
                        <div className="flex items-center gap-2 text-sm font-semibold text-[var(--text-primary)]">
                          <BookOpenText className="size-4 text-[var(--axon-primary)]" />
                          {entry[0]}
                        </div>
                        <p className="mt-2 text-sm leading-6 text-[var(--text-secondary)]">
                          {entry[1]}
                        </p>
                      </div>
                    ),
                  )}
                </div>
              </section>
            ) : null}

            {surface === 'sessions' ? (
              <section className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                {LOBES.flatMap((lobe) => [
                  `${lobe.name} · Claude · architecture sync`,
                  `${lobe.name} · Codex · implementation pass`,
                  `${lobe.name} · Gemini · docs sweep`,
                ]).map((title) => (
                  <div
                    key={title}
                    className="rounded-[24px] border border-[rgba(135,175,255,0.12)] bg-[rgba(7,12,26,0.72)] p-4"
                  >
                    <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.28em] text-[var(--text-dim)]">
                      <Bot className="size-4 text-[var(--axon-primary)]" />
                      <span>Session</span>
                    </div>
                    <p className="mt-3 text-sm leading-6 text-[var(--text-primary)]">{title}</p>
                  </div>
                ))}
              </section>
            ) : null}

            {surface === 'ops' ? (
              <section className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                {OPS_SURFACES.map(([title, description]) => (
                  <div
                    key={title}
                    className="rounded-[24px] border border-[rgba(135,175,255,0.12)] bg-[rgba(7,12,26,0.72)] p-4"
                  >
                    <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.28em] text-[var(--text-dim)]">
                      {title === 'Terminal' ? (
                        <TerminalSquare className="size-4 text-[var(--axon-primary)]" />
                      ) : title === 'Logs' ? (
                        <Logs className="size-4 text-[var(--axon-primary)]" />
                      ) : title === 'Jobs' ? (
                        <GitBranch className="size-4 text-[var(--axon-primary)]" />
                      ) : title === 'MCP' ? (
                        <Settings2 className="size-4 text-[var(--axon-primary)]" />
                      ) : (
                        <FolderTree className="size-4 text-[var(--axon-primary)]" />
                      )}
                      <span>{title}</span>
                    </div>
                    <p className="mt-3 text-sm leading-7 text-[var(--text-secondary)]">
                      {description}
                    </p>
                  </div>
                ))}
              </section>
            ) : null}
          </div>
        </section>
      </div>
    </RebootFrame>
  )
}
