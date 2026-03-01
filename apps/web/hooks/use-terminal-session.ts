'use client'

import { useCallback, useEffect, useRef, useState } from 'react'
import type { TerminalHandle } from '@/components/terminal/terminal-emulator'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { TerminalHistory } from '@/lib/terminal-history'
import type { WsServerMsg } from '@/lib/ws-protocol'

// ---------------------------------------------------------------------------
// ANSI color helpers
// ---------------------------------------------------------------------------

const ANSI = {
  reset: '\x1b[0m',
  dim: '\x1b[2m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  cyan: '\x1b[36m',
  brightBlue: '\x1b[94m',
} as const

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface UseTerminalSessionOptions {
  terminalRef: React.RefObject<TerminalHandle | null>
}

interface UseTerminalSessionReturn {
  /** True while a command is executing (between command.start and command.done/error). */
  isRunning: boolean
  /** The exec_id of the currently running command, assigned by the server on command.start. */
  currentExecId: string | null
  /** Persistent command history instance (stable ref). */
  history: TerminalHistory
  /** Parse and execute a raw input line typed by the user. */
  submitInput: (rawInput: string) => void
  /** Cancel the currently running command, if any. */
  cancelCurrent: () => void
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useTerminalSession({
  terminalRef,
}: UseTerminalSessionOptions): UseTerminalSessionReturn {
  const { send, subscribe } = useAxonWs()

  const [isRunning, setIsRunning] = useState(false)
  const [currentExecId, setCurrentExecId] = useState<string | null>(null)

  // Stable history instance — never recreated across renders.
  const historyRef = useRef<TerminalHistory>(new TerminalHistory())

  // Track the exec_id in a ref too so the WS handler closure always has the
  // current value without needing to be recreated.
  const currentExecIdRef = useRef<string | null>(null)

  // Keep a ref to isRunning for use inside the subscription callback.
  const isRunningRef = useRef(false)

  // Convenience: write a line to the terminal (appends \r\n).
  const writeln = useCallback(
    (line: string) => {
      terminalRef.current?.write(`${line}\r\n`)
    },
    [terminalRef],
  )

  // ---------------------------------------------------------------------------
  // WS subscription — subscribe once, clean up on unmount
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const unsubscribe = subscribe((msg: WsServerMsg) => {
      const term = terminalRef.current
      if (!term) return

      switch (msg.type) {
        case 'command.start': {
          const execId = msg.data.ctx.exec_id
          currentExecIdRef.current = execId
          setCurrentExecId(execId)
          isRunningRef.current = true
          setIsRunning(true)
          break
        }

        case 'command.output.line': {
          term.write(`${msg.data.line}\r\n`)
          break
        }

        case 'command.done': {
          const { exit_code, elapsed_ms } = msg.data.payload
          const ok = exit_code === 0
          const elapsed = elapsed_ms !== undefined ? ` ${(elapsed_ms / 1000).toFixed(2)}s` : ''
          if (ok) {
            writeln(`${ANSI.green}✓ done${elapsed}${ANSI.reset}`)
          } else {
            writeln(`${ANSI.red}✗ exit ${exit_code}${elapsed}${ANSI.reset}`)
          }
          currentExecIdRef.current = null
          setCurrentExecId(null)
          isRunningRef.current = false
          setIsRunning(false)
          break
        }

        case 'command.error': {
          const { message, elapsed_ms } = msg.data.payload
          const elapsed = elapsed_ms !== undefined ? ` ${(elapsed_ms / 1000).toFixed(2)}s` : ''
          writeln(`${ANSI.red}error: ${message}${elapsed}${ANSI.reset}`)
          currentExecIdRef.current = null
          setCurrentExecId(null)
          isRunningRef.current = false
          setIsRunning(false)
          break
        }

        case 'log': {
          writeln(`${ANSI.dim}${msg.line}${ANSI.reset}`)
          break
        }

        default:
          break
      }
    })

    return unsubscribe
  }, [subscribe, writeln, terminalRef])

  // ---------------------------------------------------------------------------
  // submitInput
  // ---------------------------------------------------------------------------

  const submitInput = useCallback(
    (rawInput: string) => {
      const trimmed = rawInput.trim()

      // Always move to a new line after the user presses Enter.
      terminalRef.current?.write('\r\n')

      if (!trimmed) {
        return
      }

      // Add to persistent history.
      historyRef.current.push(trimmed)

      // Parse: first token = mode, remainder = input for that mode.
      const tokens = trimmed.split(/\s+/)
      const mode = tokens[0] ?? ''
      const input = tokens.slice(1).join(' ')

      // Send the execute message. The server will respond with command.start
      // (containing the real exec_id) followed by output lines and command.done.
      send({
        type: 'execute',
        mode,
        input,
        flags: {},
      })
    },
    [send, terminalRef],
  )

  // ---------------------------------------------------------------------------
  // cancelCurrent
  // ---------------------------------------------------------------------------

  const cancelCurrent = useCallback(() => {
    const execId = currentExecIdRef.current
    if (!isRunningRef.current || !execId) return
    send({ type: 'cancel', id: execId })
  }, [send])

  return {
    isRunning,
    currentExecId,
    history: historyRef.current,
    submitInput,
    cancelCurrent,
  }
}
