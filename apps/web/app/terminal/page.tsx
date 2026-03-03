'use client'

import { useCallback, useEffect, useRef, useState } from 'react'
import type { TerminalHandle } from '@/components/terminal/terminal-emulator'
import { TerminalEmulatorWrapper } from '@/components/terminal/terminal-emulator-wrapper'
import { TerminalToolbar } from '@/components/terminal/terminal-toolbar'
import { useShellSession } from '@/hooks/use-shell-session'

export default function TerminalPage() {
  const terminalRef = useRef<TerminalHandle | null>(null)
  const [searchVisible, setSearchVisible] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')

  // Shell session — dedicated /ws/shell WebSocket, no mode routing
  const { status, sendInput, resize } = useShellSession({
    onOutput: (data) => terminalRef.current?.write(data),
  })

  // Forward raw xterm keystrokes/sequences directly to the PTY
  const handleData = useCallback(
    (data: string) => {
      sendInput(data)
    },
    [sendInput],
  )

  // Notify PTY when xterm dimensions change (FitAddon fires this after fit())
  const handleResize = useCallback(
    (cols: number, rows: number) => {
      resize(cols, rows)
    },
    [resize],
  )

  useEffect(() => {
    document.title = 'Terminal — Axon'
    const timer = setTimeout(() => terminalRef.current?.focus(), 200)
    return () => clearTimeout(timer)
  }, [])

  const handleClear = useCallback(() => terminalRef.current?.clear(), [])

  const handleCopy = useCallback(() => {
    const text = terminalRef.current?.getSelectedText() ?? ''
    if (text) {
      navigator.clipboard.writeText(text).catch(() => {
        /* ignore clipboard errors */
      })
    }
  }, [])

  const handleSearchChange = useCallback((val: string) => {
    setSearchQuery(val)
    if (val) terminalRef.current?.search(val)
  }, [])

  const handleToggleSearch = useCallback(() => setSearchVisible((prev) => !prev), [])

  return (
    <div className="relative flex h-screen flex-col overflow-hidden">
      {/* Toolbar */}
      <header className="relative z-30 flex-shrink-0">
        <TerminalToolbar
          status={status}
          isRunning={false}
          onClear={handleClear}
          onCopy={handleCopy}
          onCancelCurrent={() => {}}
          searchVisible={searchVisible}
          onToggleSearch={handleToggleSearch}
        />
      </header>

      {/* Terminal area */}
      <main className="relative z-10 flex flex-1 flex-col overflow-hidden p-2">
        <div
          className="relative flex-1 overflow-hidden rounded-xl border"
          style={{
            background: 'rgba(3,7,18,0.95)',
            borderColor: 'var(--axon-border, rgba(255,135,175,0.12))',
          }}
        >
          {/* Search overlay */}
          {searchVisible && (
            <div
              className="absolute right-3 top-2 z-20 flex items-center gap-1 rounded-md border px-2 py-1"
              style={{
                background: 'rgba(9,18,37,0.95)',
                borderColor: 'rgba(175,215,255,0.2)',
              }}
            >
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => handleSearchChange(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Escape') {
                    setSearchVisible(false)
                    setSearchQuery('')
                    terminalRef.current?.focus()
                  }
                  if (e.key === 'Enter') {
                    terminalRef.current?.search(searchQuery)
                  }
                }}
                placeholder="Search..."
                className="w-40 bg-transparent font-mono text-xs outline-none"
                style={{ color: 'var(--text-primary)' }}
                aria-label="Terminal search"
              />
              <button
                type="button"
                onClick={() => {
                  setSearchVisible(false)
                  setSearchQuery('')
                  terminalRef.current?.focus()
                }}
                className="ml-1 text-xs"
                style={{ color: 'var(--text-muted)' }}
                aria-label="Close search"
              >
                ✕
              </button>
            </div>
          )}

          {/* xterm.js terminal — onResize notifies PTY of dimension changes */}
          <TerminalEmulatorWrapper
            ref={terminalRef}
            onData={handleData}
            onResize={handleResize}
            className="h-full w-full"
          />
        </div>
      </main>
    </div>
  )
}
