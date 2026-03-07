'use client'

import dynamic from 'next/dynamic'
import type { ReactNode } from 'react'

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), {
  ssr: false,
})

export function RebootFrame({ children }: { children: ReactNode }) {
  return (
    <main className="relative min-h-screen overflow-hidden bg-[#030817] text-[var(--text-primary)]">
      <NeuralCanvas profile="current" />
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(135,175,255,0.16),transparent_26%),radial-gradient(circle_at_80%_15%,rgba(255,135,175,0.12),transparent_20%),linear-gradient(180deg,rgba(3,8,23,0.46),rgba(3,8,23,0.9))]" />
      <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(rgba(135,175,255,0.03)_1px,transparent_1px),linear-gradient(90deg,rgba(135,175,255,0.03)_1px,transparent_1px)] bg-[size:44px_44px] opacity-35" />
      <div className="relative z-[1]">{children}</div>
    </main>
  )
}
