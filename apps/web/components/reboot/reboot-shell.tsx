'use client'

import { FolderKanban, PanelLeft, Sparkles } from 'lucide-react'
import { RebootPanel, RebootRouteLink, RebootScene } from './reboot-scene'

export function RebootShell() {
  return (
    <RebootScene>
      <div className="flex min-h-screen flex-col justify-center">
        <div className="mb-6 flex items-center gap-2 text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
          <Sparkles className="size-4 text-[var(--axon-primary)]" />
          <span>Axon reboot</span>
        </div>
        <h1 className="max-w-5xl text-[clamp(2.6rem,5vw,5.2rem)] font-semibold tracking-[-0.07em] text-[var(--text-primary)]">
          Two shells. One project home base, one live work surface.
        </h1>
        <p className="mt-5 max-w-3xl text-base leading-8 text-[var(--text-secondary)]">
          The reboot now splits cleanly: `Lobe` for repo-scoped intelligence, planning, docs,
          sessions, and project state. `Workflow` for the fluid session sidebar, conversation, and
          editor loop.
        </p>

        <div className="mt-8 grid gap-4 xl:grid-cols-2">
          <RebootRouteLink
            href="/reboot/lobe"
            title="Lobe shell"
            copy="Create or load a lobe, seed the repo with sessions and docs, then use the dashboard as the project’s living home base."
          />
          <RebootRouteLink
            href="/reboot/workflow"
            title="Workflow shell"
            copy="Work across multiple repos at once with a global session rail, a focused conversation lane, a contextual editor pane, and a bottom terminal drawer."
          />
        </div>

        <RebootPanel className="mt-6 p-5">
          <div className="grid gap-4 md:grid-cols-2">
            <div className="rounded-[24px] border border-[var(--border-subtle)] bg-[rgba(7,12,26,0.74)] p-4">
              <div className="flex items-center gap-2 text-sm font-medium text-[var(--text-primary)]">
                <FolderKanban className="size-4 text-[var(--axon-secondary)]" />
                <span>Lobe</span>
              </div>
              <p className="mt-3 text-sm leading-7 text-[var(--text-secondary)]">
                Repo, branches, PRs, reviews, issues, docs, sessions, AI config, Qdrant, CI, logs,
                todos, roadmap, notes, and jobs. Dense capability, but organized around project
                state instead of a cramped three-pane layout.
              </p>
            </div>
            <div className="rounded-[24px] border border-[var(--border-subtle)] bg-[rgba(7,12,26,0.74)] p-4">
              <div className="flex items-center gap-2 text-sm font-medium text-[var(--text-primary)]">
                <PanelLeft className="size-4 text-[var(--axon-primary)]" />
                <span>Workflow</span>
              </div>
              <p className="mt-3 text-sm leading-7 text-[var(--text-secondary)]">
                Sidebar only, sidebar plus chat, chat plus editor, or all three together. The shell
                stays adaptive so the work can move from omnibox to implementation without friction.
              </p>
            </div>
          </div>
        </RebootPanel>
      </div>
    </RebootScene>
  )
}
