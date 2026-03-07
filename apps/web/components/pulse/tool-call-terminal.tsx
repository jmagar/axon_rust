'use client'

import { CheckCircle2, ChevronDown, ChevronRight, Loader2, XCircle } from 'lucide-react'
import dynamic from 'next/dynamic'
import { useCallback, useEffect, useRef, useState } from 'react'
import type { TerminalHandle } from '@/components/terminal/terminal-emulator'

const TerminalEmulator = dynamic(
  () => import('@/components/terminal/terminal-emulator').then((m) => m.TerminalEmulator),
  { ssr: false },
)

interface ToolCallTerminalProps {
  toolName: string
  toolCallId: string
  input?: Record<string, unknown>
  content?: string
  status?: string
}

function StatusIcon({ status }: { status?: string }) {
  if (!status || status.includes('Running')) {
    return <Loader2 className="size-3.5 animate-spin text-[var(--axon-primary)]" />
  }
  if (status.includes('Completed') || status.includes('Success')) {
    return <CheckCircle2 className="size-3.5 text-emerald-400" />
  }
  return <XCircle className="size-3.5 text-red-400" />
}

export function ToolCallTerminal({
  toolName,
  toolCallId,
  input,
  content,
  status,
}: ToolCallTerminalProps) {
  const [expanded, setExpanded] = useState(true)
  const termRef = useRef<TerminalHandle>(null)
  const writtenLenRef = useRef(0)

  useEffect(() => {
    if (!content || !termRef.current) return
    const newContent = content.slice(writtenLenRef.current)
    if (newContent) {
      termRef.current.write(newContent)
      writtenLenRef.current = content.length
    }
  }, [content])

  useEffect(() => {
    if (
      status &&
      (status.includes('Completed') || status.includes('Success')) &&
      content &&
      content.length > 500
    ) {
      setExpanded(false)
    }
  }, [status, content])

  const noopOnData = useCallback(() => {}, [])

  const inputSummary = input
    ? Object.entries(input)
        .map(([k, v]) => {
          const val = typeof v === 'string' ? v.slice(0, 80) : JSON.stringify(v).slice(0, 80)
          return `${k}: ${val}`
        })
        .join(', ')
        .slice(0, 120)
    : ''

  return (
    <div
      className="my-1.5 overflow-hidden rounded-md border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.6)]"
      data-tool-call-id={toolCallId}
    >
      <button
        type="button"
        onClick={() => setExpanded((prev) => !prev)}
        className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[length:var(--text-sm)] transition-colors hover:bg-[var(--surface-float)]"
      >
        {expanded ? (
          <ChevronDown className="size-3.5 text-[var(--text-dim)]" />
        ) : (
          <ChevronRight className="size-3.5 text-[var(--text-dim)]" />
        )}
        <StatusIcon status={status} />
        <span className="font-medium text-[var(--text-secondary)]">{toolName}</span>
        {inputSummary && <span className="truncate text-[var(--text-dim)]">{inputSummary}</span>}
      </button>

      {expanded && content && (
        <div className="h-48 border-t border-[var(--border-subtle)]">
          <TerminalEmulator ref={termRef} onData={noopOnData} className="h-full w-full" />
        </div>
      )}
    </div>
  )
}
