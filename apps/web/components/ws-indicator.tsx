'use client'

import { useAxonWs } from '@/hooks/use-axon-ws'

export function WsIndicator() {
  const { status, statusLabel } = useAxonWs()

  return (
    <div className="fixed bottom-4 right-4 z-10 flex items-center gap-2 rounded-full bg-card/80 px-3 py-1.5 text-xs font-mono backdrop-blur-sm border border-border/50">
      <span
        className={`size-2 rounded-full ${
          status === 'connected'
            ? 'bg-emerald-400'
            : status === 'reconnecting'
              ? 'bg-amber-400 animate-pulse'
              : 'bg-red-400'
        }`}
      />
      <span className="text-muted-foreground">{statusLabel}</span>
    </div>
  )
}
