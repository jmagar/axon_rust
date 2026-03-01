'use client'

import type { ReactNode } from 'react'
import { AppShell } from '@/components/app-shell'
import { TooltipProvider } from '@/components/ui/tooltip'
import { AxonWsContext, useAxonWsProvider } from '@/hooks/use-axon-ws'
import { useWsMessagesProvider, WsMessagesContext } from '@/hooks/use-ws-messages'

export function Providers({ children }: { children: ReactNode }) {
  const ws = useAxonWsProvider()
  return (
    <AxonWsContext value={ws}>
      <WsMessagesProvider>{children}</WsMessagesProvider>
    </AxonWsContext>
  )
}

function WsMessagesProvider({ children }: { children: ReactNode }) {
  const messages = useWsMessagesProvider()
  return (
    <WsMessagesContext value={messages}>
      <TooltipProvider>
        <AppShell>{children}</AppShell>
      </TooltipProvider>
    </WsMessagesContext>
  )
}
