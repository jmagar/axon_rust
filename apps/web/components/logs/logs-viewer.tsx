'use client'

import { useVirtualizer } from '@tanstack/react-virtual'
import { ScrollText } from 'lucide-react'
import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { type LogEntry, LogLine } from './log-line'
import {
  type IndividualService,
  LogsToolbar,
  SERVICES,
  type ServiceName,
  TAIL_OPTIONS,
  type TailLines,
} from './logs-toolbar'

const MAX_LINES = 1200
const API_TOKEN = process.env.NEXT_PUBLIC_AXON_API_TOKEN
const LOGS_SERVICE_KEY = 'axon.web.logs.service'
const DEFAULT_SERVICE: ServiceName = 'axon-web'

export function LogsViewer() {
  const router = useRouter()
  const [service, setService] = useState<ServiceName>(DEFAULT_SERVICE)
  const [tailLines, setTailLines] = useState<TailLines>(TAIL_OPTIONS[1]) // 100
  const [lines, setLines] = useState<LogEntry[]>([])
  const [filter, setFilter] = useState('')
  const [autoScroll, setAutoScroll] = useState(true)
  const [compact, setCompact] = useState(true)
  const [wrapLines, setWrapLines] = useState(false)
  const [isConnected, setIsConnected] = useState(false)

  const scrollAreaRef = useRef<HTMLDivElement>(null)
  const autoScrollRef = useRef(autoScroll)

  useEffect(() => {
    autoScrollRef.current = autoScroll
  }, [autoScroll])

  useEffect(() => {
    try {
      const saved = window.localStorage.getItem(LOGS_SERVICE_KEY)
      if (saved && (SERVICES.includes(saved as IndividualService) || saved === 'all')) {
        setService(saved as ServiceName)
      }
    } catch {
      /* ignore */
    }
  }, [])

  // SSE connection — reconnects when service or tailLines changes
  useEffect(() => {
    setLines([])
    setIsConnected(false)

    const params = new URLSearchParams({
      service,
      tail: String(tailLines),
    })
    if (API_TOKEN) params.set('token', API_TOKEN)
    const es = new EventSource(`/api/logs?${params.toString()}`)

    es.onopen = () => setIsConnected(true)
    es.onerror = () => setIsConnected(false)

    es.onmessage = (e: MessageEvent<string>) => {
      try {
        const {
          line,
          ts,
          service: svc,
        } = JSON.parse(e.data) as {
          line: string
          ts: number
          service?: string
        }
        const newEntry: LogEntry = { text: line, ts, ...(svc ? { service: svc } : {}) }
        setLines((prev) => {
          if (prev.length >= MAX_LINES) {
            const trimmed = prev.slice(prev.length - MAX_LINES + 1)
            trimmed.push(newEntry)
            return trimmed
          }
          return [...prev, newEntry]
        })
      } catch {
        // malformed SSE data — ignore
      }
    }

    return () => {
      es.close()
      setIsConnected(false)
    }
  }, [service, tailLines])

  // Pause auto-scroll when user manually scrolls up
  const handleScroll = useCallback(() => {
    const el = scrollAreaRef.current
    if (!el) return
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40
    if (!atBottom) setAutoScroll(false)
  }, [])

  const filteredLines = useMemo(() => {
    if (!filter.trim()) return lines
    const lower = filter.toLowerCase()
    return lines.filter((l) => l.text.toLowerCase().includes(lower))
  }, [lines, filter])

  const rowVirtualizer = useVirtualizer({
    count: filteredLines.length,
    getScrollElement: () => scrollAreaRef.current,
    estimateSize: () => (wrapLines ? 36 : 20),
    overscan: 30,
  })

  // Auto-scroll when new lines arrive. Depend on filteredLines (array ref)
  // instead of filteredLines.length so the effect fires even when length is
  // constant (entries rotating at MAX_LINES — useMemo produces a new ref).
  useEffect(() => {
    if (autoScrollRef.current && filteredLines.length > 0) {
      rowVirtualizer.scrollToIndex(filteredLines.length - 1)
    }
  }, [filteredLines, rowVirtualizer])

  function handleServiceChange(s: ServiceName) {
    setService(s)
    try {
      window.localStorage.setItem(LOGS_SERVICE_KEY, s)
    } catch {
      /* ignore */
    }
  }

  function handleTailChange(t: TailLines) {
    setTailLines(t)
  }

  function handleAutoScrollToggle() {
    const next = !autoScroll
    setAutoScroll(next)
    if (next && filteredLines.length > 0) {
      rowVirtualizer.scrollToIndex(filteredLines.length - 1)
    }
  }

  return (
    <div
      className="flex min-h-dvh flex-col"
      style={{
        background:
          'radial-gradient(ellipse at 14% 10%, rgba(175,215,255,0.08), transparent 34%), radial-gradient(ellipse at 82% 16%, rgba(255,135,175,0.07), transparent 38%), linear-gradient(180deg,#02040b 0%,#030712 60%,#040a14 100%)',
      }}
    >
      {/* Top bar */}
      <header
        className="sticky top-0 z-30 flex shrink-0 items-center gap-3 border-b px-4"
        style={{
          borderColor: 'var(--border-subtle)',
          background: 'rgba(3,7,18,0.9)',
          backdropFilter: 'blur(16px)',
          height: '3.25rem',
        }}
      >
        <button
          type="button"
          onClick={() => router.back()}
          className="flex min-h-[44px] items-center gap-1.5 rounded-md px-2 py-1 text-[12px] font-medium text-[var(--text-dim)] transition-colors hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)] sm:min-h-0"
          aria-label="Go back"
        >
          ← Back
        </button>
        <div className="h-4 w-px bg-[var(--border-subtle)]" />
        <div className="flex items-center gap-2">
          <ScrollText className="size-3.5 text-[var(--axon-primary-strong)]" />
          <h1 className="text-[14px] font-semibold text-[var(--text-primary)]">Logs</h1>
        </div>
      </header>

      {/* Main content */}
      <main className="relative z-10 flex flex-1 flex-col gap-3 p-4" style={{ minHeight: 0 }}>
        {/* Toolbar */}
        <div
          className="shrink-0 rounded-xl border p-3"
          style={{
            background: 'var(--surface-base)',
            borderColor: 'var(--border-subtle)',
            backdropFilter: 'blur(12px)',
          }}
        >
          <LogsToolbar
            service={service}
            tailLines={tailLines}
            filter={filter}
            autoScroll={autoScroll}
            compact={compact}
            wrapLines={wrapLines}
            isConnected={isConnected}
            onServiceChange={handleServiceChange}
            onTailChange={handleTailChange}
            onFilterChange={setFilter}
            onAutoScrollToggle={handleAutoScrollToggle}
            onCompactToggle={() => setCompact((prev) => !prev)}
            onWrapToggle={() => setWrapLines((prev) => !prev)}
            onClear={() => setLines([])}
          />
        </div>

        {/* Log area */}
        <div
          ref={scrollAreaRef}
          onScroll={handleScroll}
          className="flex-1 overflow-y-auto rounded-xl border p-3 font-mono text-xs"
          style={{
            background: 'rgba(3,7,18,0.8)',
            borderColor: 'var(--border-subtle)',
            minHeight: 0,
          }}
          role="log"
          aria-live="polite"
          aria-label={
            service === 'all' ? 'Log output for all services' : `Log output for ${service}`
          }
        >
          {filteredLines.length === 0 && (
            <div className="flex h-full items-center justify-center">
              <p className="text-[11px] text-[var(--text-dim)]">
                {isConnected ? 'Waiting for log output…' : 'Connecting…'}
              </p>
            </div>
          )}
          <div style={{ height: `${rowVirtualizer.getTotalSize()}px`, position: 'relative' }}>
            {rowVirtualizer.getVirtualItems().map((virtualRow) => {
              const entry = filteredLines[virtualRow.index]
              return (
                <div
                  key={virtualRow.key}
                  style={{
                    position: 'absolute',
                    top: 0,
                    transform: `translateY(${virtualRow.start}px)`,
                    width: '100%',
                  }}
                >
                  <LogLine entry={entry} compact={compact} wrapLines={wrapLines} />
                </div>
              )
            })}
          </div>
        </div>

        {/* Footer meta */}
        <div className="flex shrink-0 items-center gap-3">
          <span className="text-[10px] text-[var(--text-dim)]">
            {filteredLines.length.toLocaleString()} line{filteredLines.length !== 1 ? 's' : ''}
            {filter ? ' (filtered)' : ''}
          </span>
          <span className="text-[10px] text-[var(--text-dim)]">
            Snapshot {tailLines} then live stream{' '}
            {service === 'all' ? '(all services)' : `(${service})`}
          </span>
          {lines.length >= MAX_LINES && (
            <span className="text-[10px] text-[var(--axon-warning)]">
              Buffer capped at {MAX_LINES.toLocaleString()} lines
            </span>
          )}
        </div>
      </main>
    </div>
  )
}
