'use client'

import { useAxonWs } from '@/hooks/use-axon-ws'

export function WsIndicator() {
  const { status, statusLabel } = useAxonWs()

  return (
    <div
      className="fixed bottom-3 right-3 z-20 flex cursor-default select-none items-center gap-1.5 rounded-md border px-2.5 py-1 font-mono text-[9px] tracking-wider opacity-50 transition-all duration-300 hover:opacity-100"
      style={{
        background: 'rgba(3, 7, 18, 0.6)',
        borderColor:
          status === 'connected' ? 'rgba(34, 197, 94, 0.15)' : 'rgba(175, 215, 255, 0.08)',
        color: status === 'connected' ? '#64748b' : '#475569',
        letterSpacing: '0.5px',
      }}
    >
      <span
        className={`size-[5px] rounded-full transition-all duration-300 ${
          status === 'connected'
            ? 'bg-[#22c55e] shadow-[0_0_6px_rgba(34,197,94,0.5)]'
            : status === 'reconnecting'
              ? 'animate-pulse bg-[#f59e0b] shadow-[0_0_6px_rgba(245,158,11,0.4)]'
              : 'bg-[#334155]'
        }`}
      />
      <span>{statusLabel}</span>
    </div>
  )
}
