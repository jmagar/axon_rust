'use client'

import { ArrowRight, Sparkles } from 'lucide-react'
import dynamic from 'next/dynamic'
import Link from 'next/link'
import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), {
  ssr: false,
})

export function RebootScene({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <main className={cn('relative min-h-screen overflow-hidden', className)}>
      <NeuralCanvas profile="subtle" />
      <div className="pointer-events-none absolute inset-0">
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(135,175,255,0.16),transparent_28%),radial-gradient(circle_at_top_right,rgba(255,135,175,0.14),transparent_30%),linear-gradient(180deg,rgba(3,7,18,0.12),rgba(3,7,18,0.78))]" />
        <div className="absolute inset-x-0 top-0 h-40 bg-[linear-gradient(180deg,rgba(3,7,18,0.75),transparent)]" />
        <div className="absolute inset-y-0 left-0 w-40 bg-[linear-gradient(90deg,rgba(3,7,18,0.55),transparent)]" />
      </div>
      <div className="relative z-[1] mx-auto flex min-h-screen w-full max-w-[1520px] flex-col px-4 py-5 md:px-6 xl:px-8">
        {children}
      </div>
    </main>
  )
}

export function RebootPanel({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <section
      className={cn(
        'rounded-[30px] border border-[var(--border-subtle)] bg-[linear-gradient(180deg,rgba(10,18,35,0.9),rgba(5,10,22,0.82))] shadow-[var(--shadow-xl)] backdrop-blur-md',
        className,
      )}
    >
      {children}
    </section>
  )
}

export function RebootSectionHeader({
  label,
  title,
  copy,
  action,
}: {
  label: string
  title: string
  copy?: string
  action?: ReactNode
}) {
  return (
    <div className="flex flex-col gap-4 border-b border-[var(--border-subtle)] px-5 py-4 sm:flex-row sm:items-end sm:justify-between">
      <div>
        <p className="text-[10px] uppercase tracking-[0.35em] text-[var(--text-dim)]">{label}</p>
        <h2 className="mt-2 text-lg font-semibold tracking-[-0.03em] text-[var(--text-primary)]">
          {title}
        </h2>
        {copy ? (
          <p className="mt-2 max-w-2xl text-sm text-[var(--text-secondary)]">{copy}</p>
        ) : null}
      </div>
      {action}
    </div>
  )
}

export function RebootMetric({
  label,
  value,
  detail,
}: {
  label: string
  value: string
  detail: string
}) {
  return (
    <div className="rounded-[24px] border border-[var(--border-subtle)] bg-[rgba(7,12,26,0.72)] p-4">
      <p className="text-[10px] uppercase tracking-[0.3em] text-[var(--text-dim)]">{label}</p>
      <p className="mt-3 text-lg font-semibold text-[var(--text-primary)]">{value}</p>
      <p className="mt-2 text-sm text-[var(--text-muted)]">{detail}</p>
    </div>
  )
}

export function RebootChip({
  active = false,
  children,
  onClick,
}: {
  active?: boolean
  children: ReactNode
  onClick?: () => void
}) {
  const content = (
    <span
      className={cn(
        'inline-flex items-center rounded-full border px-3 py-1.5 text-xs transition-colors',
        active
          ? 'border-[rgba(255,135,175,0.24)] bg-[rgba(255,135,175,0.12)] text-[var(--text-primary)]'
          : 'border-[var(--border-subtle)] bg-[rgba(7,12,26,0.72)] text-[var(--text-muted)]',
      )}
    >
      {children}
    </span>
  )

  if (!onClick) {
    return content
  }

  return (
    <button type="button" onClick={onClick}>
      {content}
    </button>
  )
}

export function RebootRouteLink({
  href,
  title,
  copy,
}: {
  href: string
  title: string
  copy: string
}) {
  return (
    <Link
      href={href}
      className="group rounded-[28px] border border-[var(--border-subtle)] bg-[rgba(7,12,26,0.72)] p-5 transition-transform duration-200 hover:-translate-y-0.5 hover:border-[rgba(255,135,175,0.24)]"
    >
      <div className="flex items-center gap-2 text-[10px] uppercase tracking-[0.35em] text-[var(--text-dim)]">
        <Sparkles className="size-4 text-[var(--axon-primary)]" />
        <span>Prototype route</span>
      </div>
      <h3 className="mt-5 text-xl font-semibold tracking-[-0.04em] text-[var(--text-primary)]">
        {title}
      </h3>
      <p className="mt-3 text-sm leading-7 text-[var(--text-secondary)]">{copy}</p>
      <div className="mt-6 inline-flex items-center gap-2 text-sm text-[var(--text-primary)]">
        <span>Open route</span>
        <ArrowRight className="size-4 transition-transform duration-200 group-hover:translate-x-1" />
      </div>
    </Link>
  )
}
