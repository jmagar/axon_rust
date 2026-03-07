'use client'

import { ArrowRight, Layers3, PanelsTopLeft, Sparkles } from 'lucide-react'
import Link from 'next/link'
import { RebootFrame } from './reboot-frame'

const DESTINATIONS = [
  {
    href: '/reboot/lobe',
    icon: Layers3,
    label: 'Lobe shell',
    title: 'Project home base',
    description:
      'Repo-scoped dashboard for docs, sessions, issues, roadmap, todos, config, jobs, and ops.',
  },
  {
    href: '/reboot/workflow',
    icon: PanelsTopLeft,
    label: 'Workflow shell',
    title: 'Global work surface',
    description:
      'Recent sessions across projects, active conversation, contextual editor, and a terminal drawer.',
  },
] as const

export function RebootHome() {
  return (
    <RebootFrame>
      <div className="mx-auto flex min-h-screen max-w-[1180px] flex-col justify-center px-6 py-16">
        <div className="mb-6 flex items-center gap-2 text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
          <Sparkles className="size-4 text-[var(--axon-primary)]" />
          <span>Axon reboot shells</span>
        </div>
        <h1 className="max-w-4xl text-[clamp(2.8rem,5vw,5.8rem)] font-semibold tracking-[-0.06em] text-[var(--text-primary)]">
          One project home base. One active workflow surface.
        </h1>
        <p className="mt-4 max-w-3xl text-base leading-8 text-[var(--text-secondary)]">
          The Lobe stays project-scoped. The Workflow shell stays session-scoped. Both keep your
          blue and pink system language and the neural field in the background.
        </p>

        <div className="mt-10 grid gap-5 lg:grid-cols-2">
          {DESTINATIONS.map(({ href, icon: Icon, label, title, description }) => (
            <Link
              key={href}
              className="group rounded-[34px] border border-[rgba(135,175,255,0.16)] bg-[linear-gradient(180deg,rgba(10,18,35,0.92),rgba(4,8,20,0.82))] p-6 shadow-[0_18px_80px_rgba(0,0,0,0.32)] transition-transform hover:-translate-y-1"
              href={href}
            >
              <div className="flex items-center gap-3 text-[11px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
                <Icon className="size-4 text-[var(--axon-primary)]" />
                <span>{label}</span>
              </div>
              <h2 className="mt-5 text-2xl font-semibold tracking-[-0.04em] text-[var(--text-primary)]">
                {title}
              </h2>
              <p className="mt-3 max-w-xl text-sm leading-7 text-[var(--text-secondary)]">
                {description}
              </p>
              <div className="mt-8 inline-flex items-center gap-2 rounded-full border border-[rgba(255,135,175,0.18)] bg-[rgba(255,135,175,0.1)] px-4 py-2 text-sm text-[var(--text-primary)]">
                Open shell
                <ArrowRight className="size-4 transition-transform group-hover:translate-x-0.5" />
              </div>
            </Link>
          ))}
        </div>
      </div>
    </RebootFrame>
  )
}
