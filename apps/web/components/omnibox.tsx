'use client'

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { useWsMessages } from '@/hooks/use-ws-messages'
import { getCommandSpec } from '@/lib/axon-command-map'
import type { ModeCategory, ModeDefinition, WsServerMsg } from '@/lib/ws-protocol'
import {
  MODE_CATEGORY_LABELS,
  MODE_CATEGORY_ORDER,
  MODES,
  type ModeId,
  NO_INPUT_MODES,
} from '@/lib/ws-protocol'
import { CommandOptionsPanel, type CommandOptionValues } from './command-options-panel'

export function Omnibox() {
  const { send, subscribe } = useAxonWs()
  const { startExecution } = useWsMessages()
  const [mode, setMode] = useState<ModeId>('scrape')
  const [input, setInput] = useState('')
  const [isProcessing, setIsProcessing] = useState(false)
  const [statusText, setStatusText] = useState('')
  const [statusType, setStatusType] = useState<'processing' | 'done' | 'error'>('processing')
  const [dropdownOpen, setDropdownOpen] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const omniboxRef = useRef<HTMLDivElement>(null)
  const startTimeRef = useRef(0)
  const execIdRef = useRef(0)
  const [optionValues, setOptionValues] = useState<CommandOptionValues>({})

  const currentMode = MODES.find((m) => m.id === mode) ?? MODES[0]
  const hasOptions = (getCommandSpec(mode)?.commandOptions.length ?? 0) > 0

  // Group modes by category for the dropdown
  const groupedModes = useMemo(() => {
    const groups = new Map<ModeCategory, ModeDefinition[]>()
    for (const cat of MODE_CATEGORY_ORDER) {
      groups.set(cat, [])
    }
    for (const m of MODES) {
      const list = groups.get(m.category)
      if (list) list.push(m)
    }
    return groups
  }, [])

  // Close dropdown on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (omniboxRef.current && !omniboxRef.current.contains(e.target as Node)) {
        setDropdownOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [])

  // Subscribe to WS for done/error to update local status
  useEffect(() => {
    return subscribe((msg: WsServerMsg) => {
      if (msg.type === 'done') {
        setIsProcessing(false)
        const secs = (msg.elapsed_ms / 1000).toFixed(1)
        setStatusText(`${secs}s \u00b7 exit ${msg.exit_code}`)
        setStatusType(msg.exit_code === 0 ? 'done' : 'error')
      }
      if (msg.type === 'error') {
        setIsProcessing(false)
        const secs = msg.elapsed_ms ? `${(msg.elapsed_ms / 1000).toFixed(1)}s \u00b7 ` : ''
        setStatusText(`${secs}error: ${msg.message}`)
        setStatusType('error')
      }
    })
  }, [subscribe])

  const executeCommand = useCallback(
    (execMode: ModeId, execInput: string) => {
      if (isProcessing) return
      if (!execInput.trim() && !NO_INPUT_MODES.has(execMode)) return

      execIdRef.current += 1
      setIsProcessing(true)
      startTimeRef.current = Date.now()
      setStatusText('processing...')
      setStatusType('processing')

      // Build flags from option values, filtering out empty/false values
      const flags: Record<string, string> = {}
      for (const [key, val] of Object.entries(optionValues)) {
        if (val === '' || val === false) continue
        flags[key] = String(val)
      }

      send({
        type: 'execute',
        mode: execMode,
        input: execInput.trim(),
        flags,
      })

      startExecution(execMode, execInput.trim())
    },
    [isProcessing, send, startExecution, optionValues],
  )

  const execute = useCallback(() => {
    executeCommand(mode, input)
  }, [executeCommand, mode, input])

  const cancel = useCallback(() => {
    if (!isProcessing) return
    send({ type: 'cancel', id: String(execIdRef.current) })
    setIsProcessing(false)
    const elapsed = Date.now() - startTimeRef.current
    const secs = (elapsed / 1000).toFixed(1)
    setStatusText(`${secs}s \u00b7 cancelled`)
    setStatusType('error')
  }, [isProcessing, send])

  const selectMode = useCallback(
    (id: ModeId) => {
      setMode(id)
      setDropdownOpen(false)
      setOptionValues({})
      if (NO_INPUT_MODES.has(id)) {
        setTimeout(() => {
          executeCommand(id, '')
        }, 0)
      } else {
        inputRef.current?.focus()
      }
    },
    [executeCommand],
  )

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter') {
        e.preventDefault()
        execute()
      }
      if (e.key === 'Escape') {
        setDropdownOpen(false)
      }
    },
    [execute],
  )

  return (
    <div ref={omniboxRef} className="space-y-2">
      <div
        className={`relative flex items-center rounded-xl transition-all duration-300 ${
          isProcessing
            ? 'border-[rgba(255,135,175,0.4)] shadow-[0_0_20px_rgba(255,135,175,0.15)]'
            : 'border-[rgba(175,215,255,0.18)]'
        } focus-within:border-[rgba(175,215,255,0.4)] focus-within:shadow-[0_0_0_3px_rgba(175,215,255,0.08)]`}
        style={{
          background: 'rgba(10, 18, 35, 0.65)',
          borderWidth: '1.5px',
          borderStyle: 'solid',
          borderColor: isProcessing ? 'rgba(255, 135, 175, 0.4)' : 'rgba(175, 215, 255, 0.18)',
        }}
      >
        {/* Text input */}
        <input
          ref={inputRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={
            NO_INPUT_MODES.has(mode) ? `Run ${currentMode.label}...` : 'Enter URL or query...'
          }
          className="min-w-0 flex-1 bg-transparent px-4 py-3.5 font-mono text-sm text-foreground outline-none placeholder:text-[#475569]"
          disabled={isProcessing}
        />

        {/* Inline status */}
        <div
          className={`flex shrink-0 items-center gap-1.5 overflow-hidden whitespace-nowrap transition-all duration-300 ${
            statusText ? 'max-w-[320px] px-2.5 opacity-100' : 'max-w-0 px-0 opacity-0'
          }`}
        >
          <span
            className={`size-1.5 shrink-0 rounded-full ${
              statusType === 'processing'
                ? 'animate-pulse bg-[#ff87af] shadow-[0_0_8px_rgba(255,135,175,0.7)]'
                : statusType === 'done'
                  ? 'bg-[#afd7ff] shadow-[0_0_6px_rgba(175,215,255,0.5)]'
                  : 'bg-[#ef4444] shadow-[0_0_6px_rgba(239,68,68,0.5)]'
            }`}
          />
          <span className="font-mono text-[10px] tracking-wide text-[#8787af]">{statusText}</span>
        </div>

        {/* Divider */}
        <div className="h-[22px] w-px shrink-0 bg-[rgba(175,215,255,0.12)]" />

        {/* Options button */}
        {hasOptions && (
          <>
            <button
              type="button"
              onClick={() => {
                setOptionsOpen((prev) => !prev)
                setDropdownOpen(false)
              }}
              className={`flex shrink-0 items-center gap-1.5 bg-transparent px-2.5 py-2.5 text-[10px] font-semibold uppercase tracking-wider transition-colors duration-150 ${
                optionsOpen ? 'text-[#ff9ec0]' : 'text-[#8787af] hover:text-[#afd7ff]'
              }`}
              title="Command options"
            >
              <svg
                className="size-3.5 shrink-0"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={2}
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <line x1="4" y1="21" x2="4" y2="14" />
                <line x1="4" y1="10" x2="4" y2="3" />
                <line x1="12" y1="21" x2="12" y2="12" />
                <line x1="12" y1="8" x2="12" y2="3" />
                <line x1="20" y1="21" x2="20" y2="16" />
                <line x1="20" y1="12" x2="20" y2="3" />
                <line x1="2" y1="14" x2="6" y2="14" />
                <line x1="10" y1="8" x2="14" y2="8" />
                <line x1="18" y1="16" x2="22" y2="16" />
              </svg>
              <span>Options</span>
              {activeOptionCount > 0 && (
                <span className="inline-flex min-w-[14px] items-center justify-center rounded-full border border-[rgba(255,135,175,0.35)] bg-[rgba(255,135,175,0.12)] px-1 text-[9px] text-[#ff9ec0]">
                  {activeOptionCount}
                </span>
              )}
            </button>
            <div className="h-[22px] w-px shrink-0 bg-[rgba(175,215,255,0.12)]" />
          </>
        )}

        {/* Action label — click to execute */}
        <button
          type="button"
          onClick={isProcessing ? cancel : execute}
          disabled={!isProcessing && !input.trim() && !NO_INPUT_MODES.has(mode)}
          className="flex shrink-0 items-center gap-1.5 bg-transparent px-3 py-2.5 text-[11px] font-semibold uppercase tracking-wider text-[#afd7ff] transition-colors duration-150 hover:text-white disabled:opacity-40 disabled:hover:text-[#afd7ff]"
          title={isProcessing ? 'Cancel' : 'Execute'}
        >
          {isProcessing ? (
            <>
              <span className="inline-block size-2.5 animate-spin rounded-full border-[1.5px] border-[rgba(255,135,175,0.2)] border-t-[#ff87af]" />
              <span>Cancel</span>
            </>
          ) : (
            <>
              <svg
                className="size-3.5 shrink-0"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={2}
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d={currentMode.icon} />
              </svg>
              <span>{currentMode.label}</span>
            </>
          )}
        </button>

        {/* Arrow toggle — click to open mode dropdown */}
        <button
          type="button"
          onClick={() => setDropdownOpen((prev) => !prev)}
          className="flex shrink-0 items-center justify-center rounded-r-[10px] bg-transparent px-3 py-2.5 text-[#8787af] transition-colors duration-150 hover:text-[#afd7ff]"
          title="Select mode"
        >
          <svg
            className={`size-3.5 transition-transform duration-200 ${dropdownOpen ? 'rotate-90' : ''}`}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
          </svg>
        </button>

        {/* Dropdown — grouped by category */}
        <div
          className={`absolute left-0 right-0 top-[calc(100%+6px)] z-50 space-y-1 rounded-xl border border-[rgba(175,215,255,0.15)] p-2 shadow-[0_16px_48px_rgba(0,0,0,0.5),0_0_0_1px_rgba(175,215,255,0.08)] backdrop-blur-xl transition-all duration-200 ${
            dropdownOpen
              ? 'visible translate-y-0 opacity-100'
              : 'invisible -translate-y-1 opacity-0'
          }`}
          style={{ background: 'rgba(15, 23, 42, 0.95)' }}
        >
          {MODE_CATEGORY_ORDER.map((cat) => {
            const items = groupedModes.get(cat)
            if (!items || items.length === 0) return null
            return (
              <div key={cat}>
                <div className="px-2.5 pb-1 pt-1.5 text-[9px] font-bold uppercase tracking-[0.15em] text-[#5f87af]">
                  {MODE_CATEGORY_LABELS[cat]}
                </div>
                <div className="grid grid-cols-[repeat(auto-fill,minmax(130px,1fr))] gap-0.5">
                  {items.map((m) => (
                    <button
                      key={m.id}
                      type="button"
                      onClick={() => selectMode(m.id)}
                      className={`flex items-center gap-2 rounded-lg px-3 py-2 text-left text-xs font-medium transition-all duration-150 ${
                        m.id === mode
                          ? 'bg-[rgba(255,135,175,0.12)] text-[#ff87af]'
                          : 'text-[#8787af] hover:bg-[rgba(175,215,255,0.1)] hover:text-[#afd7ff]'
                      }`}
                    >
                      <svg
                        className="size-3.5 shrink-0"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth={2}
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      >
                        <path d={m.icon} />
                      </svg>
                      <span>{m.label}</span>
                    </button>
                  ))}
                </div>
              </div>
            )
          })}
        </div>

        {/* Options popover */}
        {hasOptions && (
          <div
            className={`absolute right-0 top-[calc(100%+6px)] z-50 w-[min(560px,92vw)] rounded-xl border border-[rgba(175,215,255,0.15)] p-2 shadow-[0_16px_48px_rgba(0,0,0,0.5),0_0_0_1px_rgba(175,215,255,0.08)] backdrop-blur-xl transition-all duration-200 ${
              optionsOpen
                ? 'visible translate-y-0 opacity-100'
                : 'invisible -translate-y-1 opacity-0'
            }`}
            style={{ background: 'rgba(15, 23, 42, 0.95)' }}
          >
            <CommandOptionsPanel mode={mode} values={optionValues} onChange={setOptionValues} />
          </div>
        )}
      </div>
    </div>
  )
}
