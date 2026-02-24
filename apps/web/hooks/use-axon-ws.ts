'use client'

import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react'
import type { WsClientMsg, WsServerMsg, WsStatus } from '@/lib/ws-protocol'

const BASE_BACKOFF = 1000
const MAX_BACKOFF = 30000

interface AxonWsContextValue {
  status: WsStatus
  statusLabel: string
  send: (msg: WsClientMsg) => void
  subscribe: (handler: (msg: WsServerMsg) => void) => () => void
  updateStatusLabel: (label: string) => void
}

export const AxonWsContext = createContext<AxonWsContextValue | null>(null)

export function useAxonWs() {
  const ctx = useContext(AxonWsContext)
  if (!ctx) throw new Error('useAxonWs must be used within AxonWsProvider')
  return ctx
}

export function useAxonWsProvider() {
  const [status, setStatus] = useState<WsStatus>('disconnected')
  const [statusLabel, setStatusLabel] = useState('DISCONNECTED')
  const wsRef = useRef<WebSocket | null>(null)
  const attemptsRef = useRef(0)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const handlersRef = useRef(new Set<(msg: WsServerMsg) => void>())
  const connectRef = useRef<() => void>(() => {})

  const scheduleReconnect = useCallback(() => {
    if (timerRef.current) return
    const delay = Math.min(BASE_BACKOFF * 2 ** attemptsRef.current, MAX_BACKOFF)
    attemptsRef.current++
    setStatusLabel(`RETRY ${Math.round(delay / 1000)}s`)
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

    const proto = globalThis.location?.protocol === 'https:' ? 'wss:' : 'ws:'
    const envUrl = process.env.NEXT_PUBLIC_AXON_WS_URL
    const wsUrl = envUrl || `${proto}//${globalThis.location?.host}/ws`

    try {
      const ws = new WebSocket(wsUrl)
      wsRef.current = ws

      ws.onopen = () => {
        attemptsRef.current = 0
        setStatus('connected')
        setStatusLabel('CONNECTED')
      }

      ws.onmessage = (event) => {
        try {
          const msg: WsServerMsg = JSON.parse(event.data)
          for (const handler of handlersRef.current) handler(msg)
        } catch {
          /* malformed */
        }
      }

      ws.onclose = () => {
        setStatus('reconnecting')
        scheduleReconnect()
      }

      ws.onerror = () => {
        /* onclose fires after */
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

  const send = useCallback((msg: WsClientMsg) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(msg))
    }
  }, [])

  const subscribe = useCallback((handler: (msg: WsServerMsg) => void) => {
    handlersRef.current.add(handler)
    return () => {
      handlersRef.current.delete(handler)
    }
  }, [])

  const updateStatusLabel = useCallback((label: string) => {
    setStatusLabel(label)
  }, [])

  return { status, statusLabel, send, subscribe, updateStatusLabel }
}
