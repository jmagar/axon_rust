'use client'

import { Bot, Network, Settings2 } from 'lucide-react'
import { useRouter } from 'next/navigation'
import type { ReactNode } from 'react'
import { CmdKPalette } from '@/components/cmdk-palette'
import { PulseSidebar } from './pulse/sidebar/pulse-sidebar'

export function AppShell({ children }: { children: ReactNode }) {
  const router = useRouter()

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <PulseSidebar />
      <div className="relative z-[1] min-w-0 flex-1 overflow-y-auto">{children}</div>
      <div className="fixed right-3 top-0 z-10 flex h-11 items-center gap-1">
        <button
          type="button"
          onClick={() => router.push('/mcp')}
          title="MCP Servers"
          aria-label="MCP Servers"
          className="flex items-center justify-center size-7 rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.42)] text-[var(--text-dim)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-primary-strong)] backdrop-blur-sm"
        >
          <Network className="size-3.5" />
        </button>
        <button
          type="button"
          onClick={() => router.push('/agents')}
          title="Available Agents"
          aria-label="Available Agents"
          className="flex items-center justify-center size-7 rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.42)] text-[var(--text-dim)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-primary-strong)] backdrop-blur-sm"
        >
          <Bot className="size-3.5" />
        </button>
        <button
          type="button"
          onClick={() => router.push('/settings')}
          title="Settings"
          aria-label="Open settings"
          className="flex items-center justify-center size-7 rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.42)] text-[var(--text-dim)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-primary-strong)] backdrop-blur-sm"
        >
          <Settings2 className="size-3.5" />
        </button>
      </div>
      <CmdKPalette />
    </div>
  )
}
