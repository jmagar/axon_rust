'use client'

import '@xterm/xterm/css/xterm.css'
import type { ITerminalOptions } from '@xterm/xterm'
import { forwardRef, useEffect, useImperativeHandle, useRef } from 'react'

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export interface TerminalHandle {
  write: (data: string) => void
  writeln: (data: string) => void
  clear: () => void
  focus: () => void
  search: (query: string) => boolean
  getSelectedText: () => string
  resize: () => void
}

export interface TerminalEmulatorProps {
  onData: (data: string) => void
  onResize?: (cols: number, rows: number) => void
  className?: string
}

// ---------------------------------------------------------------------------
// Terminal theme — axon dark neural-tech palette
// ---------------------------------------------------------------------------

const TERMINAL_OPTIONS: ITerminalOptions = {
  theme: {
    background: '#030712',
    foreground: '#e8f4f8',
    cursor: '#87afff',
    cursorAccent: '#030712',
    black: '#1a2940',
    brightBlack: '#4d6a8a',
    red: '#ff6b6b',
    brightRed: '#ff87af',
    green: '#82d9a0',
    brightGreen: '#9ef5b8',
    yellow: '#ffc086',
    brightYellow: '#ffd4a8',
    blue: '#87afff',
    brightBlue: '#afd7ff',
    magenta: '#ff87af',
    brightMagenta: '#ff9ec0',
    white: '#b8cfe0',
    brightWhite: '#e8f4f8',
    selectionBackground: 'rgba(135,175,255,0.25)',
    selectionForeground: '#e8f4f8',
  },
  fontFamily: '"Noto Sans Mono", "JetBrains Mono", "Fira Code", monospace',
  fontSize: 13,
  lineHeight: 1.4,
  letterSpacing: 0,
  cursorBlink: true,
  cursorStyle: 'bar',
  scrollback: 5000,
  smoothScrollDuration: 100,
  allowTransparency: true,
  convertEol: true,
}

// ---------------------------------------------------------------------------
// Scrollbar style — injected once into document head
// ---------------------------------------------------------------------------

const SCROLLBAR_STYLE_ID = 'axon-terminal-scrollbar'

function injectScrollbarStyle(): void {
  if (typeof document === 'undefined') return
  if (document.getElementById(SCROLLBAR_STYLE_ID)) return

  const style = document.createElement('style')
  style.id = SCROLLBAR_STYLE_ID
  style.textContent = `
    .xterm-viewport::-webkit-scrollbar {
      width: 2px;
    }
    .xterm-viewport::-webkit-scrollbar-track {
      background: transparent;
    }
    .xterm-viewport::-webkit-scrollbar-thumb {
      background: rgba(135,175,255,0.2);
      border-radius: 1px;
    }
    .xterm-viewport::-webkit-scrollbar-thumb:hover {
      background: rgba(135,175,255,0.4);
    }
  `
  document.head.appendChild(style)
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * Core xterm.js terminal component. Browser-only — all xterm imports happen
 * inside useEffect via dynamic `await import()` so this module is safe to
 * import on the server; the actual terminal instantiation only runs client-side.
 *
 * Export as `TerminalEmulator` (default) and `TerminalEmulatorInner` (named)
 * so the SSR wrapper can pick it up via dynamic import.
 */
export const TerminalEmulator = forwardRef<TerminalHandle, TerminalEmulatorProps>(
  function TerminalEmulator({ onData, onResize, className }, ref) {
    const containerRef = useRef<HTMLDivElement>(null)

    // Always-current ref so the stable onData wrapper registered with xterm
    // calls the latest prop without needing to re-mount the terminal.
    const onDataRef = useRef(onData)
    onDataRef.current = onData

    // These refs hold the live xterm instances so imperative handle methods
    // have stable access without capturing stale closures.
    const termRef = useRef<import('@xterm/xterm').Terminal | null>(null)
    const fitAddonRef = useRef<import('@xterm/addon-fit').FitAddon | null>(null)
    const searchAddonRef = useRef<import('@xterm/addon-search').SearchAddon | null>(null)

    // Expose imperative handle to parent
    useImperativeHandle(ref, () => ({
      write(data: string) {
        termRef.current?.write(data)
      },
      writeln(data: string) {
        termRef.current?.writeln(data)
      },
      clear() {
        termRef.current?.clear()
      },
      focus() {
        termRef.current?.focus()
      },
      search(query: string): boolean {
        if (!searchAddonRef.current) return false
        return searchAddonRef.current.findNext(query)
      },
      getSelectedText(): string {
        return termRef.current?.getSelection() ?? ''
      },
      resize() {
        fitAddonRef.current?.fit()
      },
    }))

    useEffect(() => {
      if (!containerRef.current) return

      let disposed = false
      let observer: ResizeObserver | null = null

      // Keep a local reference to the terminal and container so the cleanup
      // closure captures the right instance even if refs change.
      let terminal: import('@xterm/xterm').Terminal | null = null

      async function init() {
        const { Terminal } = await import('@xterm/xterm')
        const { FitAddon } = await import('@xterm/addon-fit')
        const { WebLinksAddon } = await import('@xterm/addon-web-links')
        const { SearchAddon } = await import('@xterm/addon-search')

        // Guard against unmount happening during the async import
        if (disposed || !containerRef.current) return

        injectScrollbarStyle()

        terminal = new Terminal(TERMINAL_OPTIONS)
        const fitAddon = new FitAddon()
        const webLinksAddon = new WebLinksAddon()
        const searchAddon = new SearchAddon()

        terminal.loadAddon(fitAddon)
        terminal.loadAddon(webLinksAddon)
        terminal.loadAddon(searchAddon)

        terminal.open(containerRef.current)
        fitAddon.fit()

        // Expose instances via refs for imperative handle
        termRef.current = terminal
        fitAddonRef.current = fitAddon
        searchAddonRef.current = searchAddon

        // Forward keyboard input through a stable wrapper so the latest
        // onData prop is always called (avoids stale closure on re-renders).
        terminal.onData((data) => onDataRef.current(data))

        // Notify parent of resize events (fired by xterm after fit)
        if (onResize) {
          terminal.onResize(({ cols, rows }) => {
            onResize(cols, rows)
          })
        }

        // Refit whenever the container changes size
        observer = new ResizeObserver(() => {
          fitAddon.fit()
        })
        observer.observe(containerRef.current)
      }

      init()

      return () => {
        disposed = true
        observer?.disconnect()
        observer = null

        // Dispose in the next microtask so any in-flight writes complete first
        const term = terminal ?? termRef.current
        if (term) {
          term.dispose()
        }
        termRef.current = null
        fitAddonRef.current = null
        searchAddonRef.current = null
      }
      // onData and onResize are intentionally excluded — they are called
      // through the live closure without needing to re-mount the terminal.
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [onResize])

    return (
      <div ref={containerRef} className={className} style={{ width: '100%', height: '100%' }} />
    )
  },
)

// Named alias for dynamic import in the wrapper
export { TerminalEmulator as TerminalEmulatorInner }

export default TerminalEmulator
