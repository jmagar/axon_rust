'use client'

import { forwardRef, useEffect, useState } from 'react'
import type { TerminalEmulatorProps, TerminalHandle } from './terminal-emulator'

// ---------------------------------------------------------------------------
// SSR-safe wrapper
//
// xterm.js is browser-only. Rather than using Next.js `dynamic()` (which
// doesn't forward refs cleanly through forwardRef components), we use a
// manual hydration guard: the inner component module is imported dynamically
// inside a useEffect so it never executes on the server. The ref is passed
// down as a regular prop once the component is available.
// ---------------------------------------------------------------------------

type TerminalEmulatorComponent = React.ForwardRefExoticComponent<
  TerminalEmulatorProps & React.RefAttributes<TerminalHandle>
>

export const TerminalEmulatorWrapper = forwardRef<TerminalHandle, TerminalEmulatorProps>(
  function TerminalEmulatorWrapper(props, ref) {
    const [TermComp, setTermComp] = useState<TerminalEmulatorComponent | null>(null)

    useEffect(() => {
      let active = true
      import('./terminal-emulator').then((m) => {
        // setState with a function that returns the component — wrap in arrow
        // so React doesn't call it as a state initialiser.
        if (active) setTermComp(() => m.TerminalEmulator as TerminalEmulatorComponent)
      })
      return () => {
        active = false
      }
    }, [])

    if (!TermComp) {
      return <TerminalLoadingPlaceholder />
    }

    return <TermComp {...props} ref={ref} />
  },
)

// ---------------------------------------------------------------------------
// Loading placeholder — shown while the xterm.js bundle is fetching
// ---------------------------------------------------------------------------

function TerminalLoadingPlaceholder() {
  return (
    <div
      className="flex h-full w-full items-center justify-center"
      style={{ background: '#030712' }}
    >
      <div
        className="h-4 w-4 animate-spin rounded-full border-2"
        style={{
          borderColor: 'rgba(135,175,255,0.3)',
          borderTopColor: '#87afff',
        }}
      />
    </div>
  )
}
