'use client'

import { useCallback, useEffect, useRef, useState } from 'react'
import type { WsStatus } from '@/lib/ws-protocol'

const BASE_BACKOFF = 1000
const MAX_BACKOFF = 30000

interface UseShellSessionOptions {
  /** Called with raw PTY output as it arrives. */
  onOutput: (data: string) => void
}

interface UseShellSessionReturn {
  /** WebSocket connection status. */
  status: WsStatus
  /** Send raw terminal input (keystrokes, escape sequences) to the PTY. */
  sendInput: (data: string) => void
  /** Notify the PTY of terminal dimension changes. */
  resize: (cols: number, rows: number) => void
}

/**
 * Manages a dedicated WebSocket connection to /ws/shell that bridges
 * a server-side PTY. All terminal I/O passes through raw JSON messages —
 * no command parsing, no mode routing.
 */
export function useShellSession({ onOutput }: UseShellSessionOptions): UseShellSessionReturn {
  const [status, setStatus] = useState<WsStatus>('disconnected')
  const wsRef = useRef<WebSocket | null>(null)
  const onOutputRef = useRef(onOutput)
  onOutputRef.current = onOutput
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const attemptsRef = useRef(0)
  const connectRef = useRef<() => void>(() => {})

  const scheduleReconnect = useCallback(() => {
    if (timerRef.current) return
    const delay = Math.min(BASE_BACKOFF * 2 ** attemptsRef.current, MAX_BACKOFF)
    attemptsRef.current++
    timerRef.current = setTimeout(() => {
      timerRef.current = null
      connectRef.current()
    }, delay)
  }, [])

  const connect = useCallback(() => {
    if (
      wsRef.current?.readyState === WebSocket.CONNECTING ||
      wsRef.current?.readyState === WebSocket.OPEN
    )
      return

    // Derive /ws/shell URL from NEXT_PUBLIC_AXON_WS_URL or window.location
    const proto = globalThis.location?.protocol === 'https:' ? 'wss:' : 'ws:'
    const envUrl = process.env.NEXT_PUBLIC_AXON_WS_URL
    const base = envUrl ? envUrl.replace(/\/ws$/, '') : `${proto}//${globalThis.location?.host}`
    const wsUrl = `${base}/ws/shell`

    try {
      const ws = new WebSocket(wsUrl)
      wsRef.current = ws

      ws.onopen = () => {
        attemptsRef.current = 0
        setStatus('connected')
      }

      ws.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data as string) as { type: string; data?: string }
          if (msg.type === 'output' && msg.data) {
            onOutputRef.current(msg.data)
          }
        } catch {
          /* malformed JSON — ignore */
        }
      }

      ws.onclose = () => {
        setStatus('reconnecting')
        scheduleReconnect()
      }

      ws.onerror = () => {
        /* onclose fires after onerror — handled there */
      }
    } catch {
      scheduleReconnect()
    }
  }, [scheduleReconnect])

  connectRef.current = connect

  useEffect(() => {
    connect()
    return () => {
      wsRef.current?.close()
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [connect])

  const sendInput = useCallback((data: string) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: 'input', data }))
    }
  }, [])

  const resize = useCallback((cols: number, rows: number) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: 'resize', cols, rows }))
    }
  }, [])

  return { status, sendInput, resize }
}
