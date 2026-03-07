'use client'

import type { ReactNode } from 'react'
import { CmdKPalette } from '@/components/cmdk-palette'
import { PulseSidebar } from './pulse/sidebar/pulse-sidebar'

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <PulseSidebar />
      <div className="relative z-[1] min-w-0 flex-1 overflow-y-auto">{children}</div>
      <CmdKPalette />
    </div>
  )
}
