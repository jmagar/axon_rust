'use client'

import type { ReactNode } from 'react'
import { PulseSidebar } from './pulse/sidebar/pulse-sidebar'

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <PulseSidebar />
      <div className="relative min-w-0 flex-1 overflow-y-auto">{children}</div>
    </div>
  )
}
