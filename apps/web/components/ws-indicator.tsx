'use client'

import { useAxonWs } from '@/hooks/use-axon-ws'

export function WsIndicator() {
  const { status, statusLabel } = useAxonWs()

  return (
    <div
      className="fixed bottom-3 right-3 z-20 flex cursor-default select-none items-center gap-1.5 rounded-md border px-2.5 py-1 font-mono text-[10px] tracking-wider opacity-50 transition-all duration-300 hover:opacity-100"
      style={{
        background: 'rgba(3, 7, 18, 0.6)',
        borderColor:
          status === 'connected' ? 'rgba(130, 217, 160, 0.18)' : 'rgba(255,135,175, 0.08)',
        color: status === 'connected' ? 'var(--text-muted)' : 'var(--text-dim)',
        letterSpacing: '0.5px',
      }}
    >
      <span
        className={`size-[5px] rounded-full transition-all duration-300 ${
          status === 'connected'
            ? 'bg-[var(--axon-success)] shadow-[0_0_6px_rgba(130,217,160,0.55)]'
            : status === 'reconnecting'
              ? 'animate-pulse bg-[var(--axon-warning)] shadow-[0_0_6px_rgba(255,192,134,0.45)]'
              : 'bg-[var(--text-dim)]'
        }`}
      />
      <span>{statusLabel}</span>
    </div>
  )
}
