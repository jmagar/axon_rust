'use client'

import type { ReactNode } from 'react'
import { TooltipProvider } from '@/components/ui/tooltip'
import { AxonWsContext, useAxonWsProvider } from '@/hooks/use-axon-ws'

export function Providers({ children }: { children: ReactNode }) {
  const ws = useAxonWsProvider()
  return (
    <AxonWsContext value={ws}>
      <TooltipProvider>{children}</TooltipProvider>
    </AxonWsContext>
  )
}
