'use client'

import dynamic from 'next/dynamic'
import { useCallback, useEffect, useRef, useState } from 'react'
import type { TerminalHandle } from '@/components/terminal/terminal-emulator'
import { TerminalEmulatorWrapper } from '@/components/terminal/terminal-emulator-wrapper'
import { TerminalToolbar } from '@/components/terminal/terminal-toolbar'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { useTerminalSession } from '@/hooks/use-terminal-session'

// ---------------------------------------------------------------------------
// Dynamic imports (browser-only)
// ---------------------------------------------------------------------------

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), { ssr: false })

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Visible character count of the prompt string "❯ " */
const PROMPT_VISIBLE_LEN = 2

/** ANSI escape for the prompt — bright blue ❯ then reset */
const PROMPT_ANSI = '\x1b[94m❯\x1b[0m '

const WELCOME_BANNER = [
  '\x1b[94m╔══════════════════════════════════════╗\x1b[0m',
  '\x1b[94m║\x1b[0m  \x1b[1mAXON\x1b[0m \x1b[2mTerminal Interface\x1b[0m            \x1b[94m║\x1b[0m',
  '\x1b[94m╚══════════════════════════════════════╝\x1b[0m',
  '\x1b[2mType a mode and input: \x1b[0m\x1b[94mscrape\x1b[0m\x1b[2m https://example.com\x1b[0m',
  '\x1b[2mAvailable: scrape, crawl, ask, query, search, research...\x1b[0m',
  '\x1b[2mCtrl+C cancel · Ctrl+K clear · ↑↓ history\x1b[0m',
].join('\r\n')

// ---------------------------------------------------------------------------
// Page component
// ---------------------------------------------------------------------------

export default function TerminalPage() {
  const terminalRef = useRef<TerminalHandle | null>(null)
  const session = useTerminalSession({ terminalRef })
  const { status } = useAxonWs()

  // Current line buffer and cursor managed as refs (no re-render needed)
  const inputRef = useRef('')

  // Search overlay state
  const [searchVisible, setSearchVisible] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')

  // ---------------------------------------------------------------------------
  // Prompt helpers
  // ---------------------------------------------------------------------------

  const writePrompt = useCallback(() => {
    terminalRef.current?.write(`\r\n${PROMPT_ANSI}`)
  }, [])

  const writePromptNoNewline = useCallback(() => {
    terminalRef.current?.write(PROMPT_ANSI)
  }, [])

  const clearCurrentLine = useCallback(() => {
    const len = inputRef.current.length
    // Carriage return, overwrite with spaces (prompt + input), return again,
    // then re-draw the prompt so the cursor is right after it.
    terminalRef.current?.write(`\r${' '.repeat(len + PROMPT_VISIBLE_LEN)}\r`)
    writePromptNoNewline()
  }, [writePromptNoNewline])

  // ---------------------------------------------------------------------------
  // Input handler (passed to xterm onData)
  // ---------------------------------------------------------------------------

  const handleData = useCallback(
    (data: string) => {
      // Enter
      if (data === '\r') {
        const cmd = inputRef.current
        inputRef.current = ''
        session.submitInput(cmd)
        // submitInput writes \r\n before the output; write the prompt after
        // a short tick so it appears after any synchronous writes.
        setTimeout(() => {
          writePrompt()
        }, 0)
        return
      }

      // Ctrl+C
      if (data === '\x03') {
        if (session.isRunning) {
          session.cancelCurrent()
        } else {
          terminalRef.current?.write('^C')
          inputRef.current = ''
          writePrompt()
        }
        return
      }

      // Ctrl+K — clear terminal
      if (data === '\x0b') {
        terminalRef.current?.clear()
        inputRef.current = ''
        writePromptNoNewline()
        return
      }

      // Backspace
      if (data === '\x7f') {
        if (inputRef.current.length > 0 && !session.isRunning) {
          inputRef.current = inputRef.current.slice(0, -1)
          terminalRef.current?.write('\b \b')
        }
        return
      }

      // Up arrow — history previous
      if (data === '\x1b[A') {
        if (session.isRunning) return
        const prev = session.history.prev()
        if (prev !== undefined) {
          clearCurrentLine()
          inputRef.current = prev
          terminalRef.current?.write(prev)
        }
        return
      }

      // Down arrow — history next
      if (data === '\x1b[B') {
        if (session.isRunning) return
        const next = session.history.next()
        clearCurrentLine()
        inputRef.current = next ?? ''
        if (next !== undefined) {
          terminalRef.current?.write(next)
        }
        return
      }

      // Ignore other escape sequences
      if (data.startsWith('\x1b')) return

      // Regular printable character — only accept when not running
      if (!session.isRunning && data >= ' ') {
        inputRef.current += data
        terminalRef.current?.write(data)
      }
    },
    [session, writePrompt, writePromptNoNewline, clearCurrentLine],
  )

  // ---------------------------------------------------------------------------
  // Welcome banner + first prompt on mount
  // ---------------------------------------------------------------------------

  useEffect(() => {
    document.title = 'Terminal — Axon'

    // Small delay to let the xterm component finish its async init
    const timer = setTimeout(() => {
      const term = terminalRef.current
      if (!term) return
      term.write(WELCOME_BANNER)
      writePrompt()
      term.focus()
    }, 200)

    return () => clearTimeout(timer)
  }, [writePrompt])

  // ---------------------------------------------------------------------------
  // Page-level keyboard shortcuts
  // ---------------------------------------------------------------------------

  useEffect(() => {
    function onKeydown(e: KeyboardEvent) {
      // Ctrl+C: cancel running command (only when terminal is not focused,
      // since the xterm onData path handles it when focused).
      // Ctrl+K is intentionally NOT handled here — xterm's onData path
      // handles it reliably and re-handling it here causes double-clears.
      if (e.ctrlKey && e.key === 'c') {
        if (session.isRunning) {
          e.preventDefault()
          session.cancelCurrent()
        }
      }
    }
    window.addEventListener('keydown', onKeydown)
    return () => window.removeEventListener('keydown', onKeydown)
  }, [session])

  // ---------------------------------------------------------------------------
  // Search query handling
  // ---------------------------------------------------------------------------

  const handleSearchChange = useCallback((val: string) => {
    setSearchQuery(val)
    if (val) {
      terminalRef.current?.search(val)
    }
  }, [])

  // ---------------------------------------------------------------------------
  // Toolbar handlers
  // ---------------------------------------------------------------------------

  const handleClear = useCallback(() => {
    terminalRef.current?.clear()
    inputRef.current = ''
    writePromptNoNewline()
  }, [writePromptNoNewline])

  const handleCopy = useCallback(() => {
    const text = terminalRef.current?.getSelectedText() ?? ''
    if (text) {
      navigator.clipboard.writeText(text).catch(() => {
        /* ignore clipboard errors */
      })
    }
  }, [])

  const handleToggleSearch = useCallback(() => {
    setSearchVisible((prev) => !prev)
  }, [])

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <div className="relative flex h-screen flex-col overflow-hidden">
      {/* Background */}
      <div className="fixed inset-0 z-0">
        <NeuralCanvas />
      </div>

      {/* Toolbar */}
      <header className="relative z-30 flex-shrink-0">
        <TerminalToolbar
          status={status}
          isRunning={session.isRunning}
          onClear={handleClear}
          onCopy={handleCopy}
          onCancelCurrent={session.cancelCurrent}
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
                style={{ color: 'var(--axon-text-primary, #e8f4f8)' }}
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
                style={{ color: 'var(--axon-text-muted, #93aaca)' }}
                aria-label="Close search"
              >
                ✕
              </button>
            </div>
          )}

          {/* xterm.js terminal */}
          <TerminalEmulatorWrapper
            ref={terminalRef}
            onData={handleData}
            className="h-full w-full"
          />
        </div>
      </main>
    </div>
  )
}
