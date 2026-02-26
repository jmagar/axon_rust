'use client'

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { useWsMessages } from '@/hooks/use-ws-messages'
import { getCommandSpec } from '@/lib/axon-command-map'
import {
  deriveOmniboxPhase,
  extractActiveMention,
  extractMentionLabels,
  getMentionKind,
  type LocalDocFile,
  type MentionKind,
  rankFileSuggestions,
  rankModeSuggestions,
  replaceActiveMention,
} from '@/lib/omnibox'
import type { ModeCategory, ModeDefinition, WsServerMsg } from '@/lib/ws-protocol'
import {
  isWorkspaceMode,
  MODE_CATEGORY_LABELS,
  MODE_CATEGORY_ORDER,
  MODES,
  type ModeId,
  NO_INPUT_MODES,
} from '@/lib/ws-protocol'
import { CommandOptionsPanel, type CommandOptionValues } from './command-options-panel'

export function Omnibox() {
  const { send, subscribe } = useAxonWs()
  const { startExecution, activateWorkspace, submitWorkspacePrompt, currentJobId } = useWsMessages()
  const [mode, setMode] = useState<ModeId>('scrape')
  const [input, setInput] = useState('')
  const [isProcessing, setIsProcessing] = useState(false)
  const [statusText, setStatusText] = useState('')
  const [statusType, setStatusType] = useState<'processing' | 'done' | 'error'>('processing')
  const [dropdownOpen, setDropdownOpen] = useState(false)
  const [optionsOpen, setOptionsOpen] = useState(false)
  const [mentionSuggestions, setMentionSuggestions] = useState<ModeDefinition[]>([])
  const [fileSuggestions, setFileSuggestions] = useState<LocalDocFile[]>([])
  const [mentionSelectionIndex, setMentionSelectionIndex] = useState(0)
  const [modeAppliedLabel, setModeAppliedLabel] = useState<string | null>(null)
  const [localDocFiles, setLocalDocFiles] = useState<LocalDocFile[]>([])
  const [fileContextMentions, setFileContextMentions] = useState<Record<string, LocalDocFile>>({})
  const [recentFileSelections, setRecentFileSelections] = useState<Record<string, number>>({})
  const inputRef = useRef<HTMLInputElement>(null)
  const omniboxRef = useRef<HTMLDivElement>(null)
  const startTimeRef = useRef(0)
  const execIdRef = useRef(0)
  const [optionValues, setOptionValues] = useState<CommandOptionValues>({})

  const currentMode = MODES.find((m) => m.id === mode) ?? MODES[0]
  const hasOptions = (getCommandSpec(mode)?.commandOptions.length ?? 0) > 0
  const activeOptionCount = useMemo(
    () => Object.values(optionValues).filter((val) => val !== '' && val !== false).length,
    [optionValues],
  )
  const activeMentionToken = useMemo(() => extractActiveMention(input), [input])
  const mentionKind: MentionKind = useMemo(
    () => getMentionKind(input, activeMentionToken),
    [input, activeMentionToken],
  )
  const activeSuggestions = mentionKind === 'mode' ? mentionSuggestions : fileSuggestions
  const omniboxPhase = useMemo(
    () =>
      deriveOmniboxPhase({
        isProcessing,
        input,
        mentionKind,
        hasModeFeedback: Boolean(modeAppliedLabel),
      }),
    [input, isProcessing, mentionKind, modeAppliedLabel],
  )

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
        setOptionsOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [])

  // Subscribe to WS for command completion updates.
  useEffect(() => {
    return subscribe((msg: WsServerMsg) => {
      if (msg.type === 'command.done') {
        setIsProcessing(false)
        const secs = ((msg.data.payload.elapsed_ms ?? 0) / 1000).toFixed(1)
        setStatusText(`${secs}s \u00b7 exit ${msg.data.payload.exit_code}`)
        setStatusType(msg.data.payload.exit_code === 0 ? 'done' : 'error')
      }
      if (msg.type === 'command.error') {
        setIsProcessing(false)
        const secs = msg.data.payload.elapsed_ms
          ? `${(msg.data.payload.elapsed_ms / 1000).toFixed(1)}s \u00b7 `
          : ''
        setStatusText(`${secs}error: ${msg.data.payload.message}`)
        setStatusType('error')
      }
      if (msg.type === 'job.cancel.response') {
        setIsProcessing(false)
        const modeLabel = msg.data.payload.mode ?? mode
        const jobLabel = msg.data.payload.job_id ? ` \u00b7 ${msg.data.payload.job_id}` : ''
        const resultMessage =
          msg.data.payload.message ?? (msg.data.payload.ok ? 'cancel accepted' : 'cancel failed')
        setStatusText(`${modeLabel}${jobLabel} \u00b7 ${resultMessage}`)
        setStatusType(msg.data.payload.ok ? 'done' : 'error')
      }
    })
  }, [mode, subscribe])

  // Global "/" shortcut to focus the omnibox.
  useEffect(() => {
    function isEditableElement(target: EventTarget | null): boolean {
      if (!(target instanceof HTMLElement)) return false
      if (target.isContentEditable) return true
      const tag = target.tagName.toLowerCase()
      return tag === 'input' || tag === 'textarea' || tag === 'select'
    }

    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== '/' || event.metaKey || event.ctrlKey || event.altKey) return
      if (isEditableElement(event.target)) return
      event.preventDefault()
      inputRef.current?.focus()
    }

    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [])

  useEffect(() => {
    let cancelled = false

    fetch('/api/omnibox/files')
      .then((res) => res.json())
      .then((data: { files?: LocalDocFile[] }) => {
        if (cancelled) return
        setLocalDocFiles(Array.isArray(data.files) ? data.files : [])
      })
      .catch(() => {
        if (!cancelled) setLocalDocFiles([])
      })

    return () => {
      cancelled = true
    }
  }, [])

  // Mention suggestions: "@mode" from beginning, "... @file" for local docs.
  useEffect(() => {
    if (!activeMentionToken) {
      setMentionSuggestions([])
      setFileSuggestions([])
      setMentionSelectionIndex(0)
      return
    }

    if (mentionKind === 'mode') {
      const matches = rankModeSuggestions(MODES, activeMentionToken.query, 3)
      setMentionSuggestions(matches)
      setFileSuggestions([])
      setMentionSelectionIndex(0)
      return
    }

    const matches = rankFileSuggestions(
      localDocFiles,
      activeMentionToken.query,
      recentFileSelections,
      3,
    )
    setFileSuggestions(matches)
    setMentionSuggestions([])
    setMentionSelectionIndex(0)
  }, [activeMentionToken, localDocFiles, mentionKind, recentFileSelections])

  const buildInputWithFileContext = useCallback(
    async (rawInput: string) => {
      const mentionLabels = extractMentionLabels(rawInput)
      const matchingFiles = mentionLabels
        .map((label) => fileContextMentions[label.toLowerCase()])
        .filter((file): file is LocalDocFile => Boolean(file))
        .slice(0, 3)

      if (matchingFiles.length === 0) {
        return {
          enrichedInput: rawInput.trim(),
          contextFileLabels: [] as string[],
        }
      }

      const contextBlocks = await Promise.all(
        matchingFiles.map(async (file) => {
          try {
            const res = await fetch(`/api/omnibox/files?id=${encodeURIComponent(file.id)}`)
            if (!res.ok) return null
            const data = (await res.json()) as {
              file?: { content?: string; label?: string }
            }
            const content = data.file?.content?.trim()
            if (!content) return null
            const label = data.file?.label ?? file.label
            return `### ${label}\n${content.slice(0, 2400)}`
          } catch {
            return null
          }
        }),
      )

      const usableBlocks = contextBlocks.filter((block): block is string => Boolean(block))
      if (usableBlocks.length === 0) {
        return {
          enrichedInput: rawInput.trim(),
          contextFileLabels: [] as string[],
        }
      }

      const contextSection = `\n\nLocal file context:\n${usableBlocks.join('\n\n---\n\n')}`
      return {
        enrichedInput: `${rawInput.trim()}${contextSection}`,
        contextFileLabels: matchingFiles.map((file) => file.label),
      }
    },
    [fileContextMentions],
  )

  const executeCommand = useCallback(
    async (execMode: ModeId, execInput: string) => {
      if (isProcessing) return

      const trimmedInput = execInput.trim()
      if (!trimmedInput && !NO_INPUT_MODES.has(execMode)) return

      const { enrichedInput, contextFileLabels } = await buildInputWithFileContext(trimmedInput)

      if (isWorkspaceMode(execMode)) {
        activateWorkspace(execMode)
        if (enrichedInput) submitWorkspacePrompt(enrichedInput)
        return
      }

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
      if (contextFileLabels.length > 0) {
        flags.context_files = contextFileLabels.join(',')
      }

      send({
        type: 'execute',
        mode: execMode,
        input: enrichedInput,
        flags,
      })

      startExecution(execMode, enrichedInput)
    },
    [
      isProcessing,
      buildInputWithFileContext,
      activateWorkspace,
      submitWorkspacePrompt,
      send,
      startExecution,
      optionValues,
    ],
  )

  const execute = useCallback(() => {
    void executeCommand(mode, input)
  }, [executeCommand, mode, input])

  const cancel = useCallback(() => {
    if (!isProcessing) return
    const fallbackId = String(execIdRef.current)
    const cancelId = currentJobId ?? fallbackId
    send({
      type: 'cancel',
      id: cancelId,
      mode,
      job_id: currentJobId ?? undefined,
    })
    setIsProcessing(false)
    const elapsed = Date.now() - startTimeRef.current
    const secs = (elapsed / 1000).toFixed(1)
    setStatusText(`${secs}s \u00b7 cancelled`)
    setStatusType('error')
  }, [currentJobId, isProcessing, mode, send])

  const selectMode = useCallback(
    (id: ModeId) => {
      setMode(id)
      setDropdownOpen(false)
      setOptionsOpen(false)
      setOptionValues({})
      if (isWorkspaceMode(id)) {
        activateWorkspace(id)
      } else if (NO_INPUT_MODES.has(id)) {
        setTimeout(() => {
          void executeCommand(id, '')
        }, 0)
      } else {
        inputRef.current?.focus()
      }
    },
    [activateWorkspace, executeCommand],
  )

  const applyModeMentionCandidate = useCallback(
    (candidate: ModeDefinition) => {
      selectMode(candidate.id as ModeId)
      setInput('')
      setMentionSuggestions([])
      setFileSuggestions([])
      setMentionSelectionIndex(0)
      setModeAppliedLabel(candidate.label)
      return true
    },
    [selectMode],
  )

  const applyFileMentionCandidate = useCallback(
    (candidate: LocalDocFile) => {
      if (!activeMentionToken) return false
      const nextInput = replaceActiveMention(input, activeMentionToken, `@${candidate.label} `)
      setInput(nextInput)
      setFileSuggestions([])
      setMentionSuggestions([])
      setMentionSelectionIndex(0)
      setFileContextMentions((prev) => ({
        ...prev,
        [candidate.label.toLowerCase()]: candidate,
      }))
      setRecentFileSelections((prev) => ({
        ...prev,
        [candidate.id]: Date.now(),
      }))
      return true
    },
    [activeMentionToken, input],
  )

  const applyActiveSuggestion = useCallback(() => {
    if (mentionKind === 'mode') {
      const selected = mentionSuggestions[mentionSelectionIndex]
      return selected ? applyModeMentionCandidate(selected) : false
    }
    const selected = fileSuggestions[mentionSelectionIndex]
    return selected ? applyFileMentionCandidate(selected) : false
  }, [
    mentionSelectionIndex,
    mentionKind,
    mentionSuggestions,
    fileSuggestions,
    applyModeMentionCandidate,
    applyFileMentionCandidate,
  ])

  useEffect(() => {
    if (!modeAppliedLabel) return
    const timer = setTimeout(() => setModeAppliedLabel(null), 900)
    return () => clearTimeout(timer)
  }, [modeAppliedLabel])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const hasMentionSelection = activeSuggestions.length > 0 && mentionKind !== 'none'
      if (e.key === 'ArrowDown' && hasMentionSelection) {
        e.preventDefault()
        setMentionSelectionIndex((prev) => (prev + 1) % activeSuggestions.length)
        return
      }
      if (e.key === 'ArrowUp' && hasMentionSelection) {
        e.preventDefault()
        setMentionSelectionIndex(
          (prev) => (prev - 1 + activeSuggestions.length) % activeSuggestions.length,
        )
        return
      }
      if (e.key === 'Tab' && hasMentionSelection) {
        e.preventDefault()
        applyActiveSuggestion()
        return
      }
      if (e.key === 'Enter') {
        if (hasMentionSelection) {
          e.preventDefault()
          applyActiveSuggestion()
          return
        }
        e.preventDefault()
        execute()
      }
      if (e.key === 'Escape') {
        setDropdownOpen(false)
        setOptionsOpen(false)
        setMentionSuggestions([])
        setFileSuggestions([])
      }
    },
    [activeSuggestions, mentionKind, applyActiveSuggestion, execute],
  )

  return (
    <div ref={omniboxRef} className="space-y-2">
      <div
        className={`relative flex items-center rounded-xl transition-all duration-300 ${
          isProcessing
            ? 'border-[rgba(175,215,255,0.4)] shadow-[0_0_20px_rgba(175,215,255,0.15)]'
            : 'border-[rgba(255,135,175,0.18)]'
        } focus-within:border-[rgba(255,135,175,0.4)] focus-within:shadow-[0_0_0_3px_rgba(255,135,175,0.08)]`}
        style={{
          background: 'rgba(10, 18, 35, 0.65)',
          borderWidth: '1.5px',
          borderStyle: 'solid',
          borderColor: isProcessing ? 'rgba(175,215,255, 0.4)' : 'rgba(255,135,175, 0.18)',
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
          className="min-w-0 flex-1 bg-transparent px-4 py-3.5 font-mono text-sm text-foreground outline-none placeholder:text-[var(--axon-text-subtle)]"
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
                ? 'animate-pulse bg-[var(--axon-accent-pink)] shadow-[0_0_8px_rgba(175,215,255,0.7)]'
                : statusType === 'done'
                  ? 'bg-[var(--axon-accent-blue)] shadow-[0_0_6px_rgba(255,135,175,0.5)]'
                  : 'bg-[#ef4444] shadow-[0_0_6px_rgba(239,68,68,0.5)]'
            }`}
          />
          <span className="font-mono text-[11px] tracking-wide text-[var(--axon-text-muted)]">
            {statusText}
          </span>
        </div>

        {/* Divider */}
        <div className="h-[22px] w-px shrink-0 bg-[rgba(255,135,175,0.12)]" />

        {/* Options button */}
        {hasOptions && (
          <>
            <button
              type="button"
              onClick={() => {
                setOptionsOpen((prev) => !prev)
                setDropdownOpen(false)
              }}
              className={`flex shrink-0 items-center gap-1.5 bg-transparent px-2.5 py-2.5 text-[11px] font-semibold uppercase tracking-wider transition-colors duration-150 ${
                optionsOpen
                  ? 'text-[var(--axon-accent-pink-strong)]'
                  : 'text-[var(--axon-text-muted)] hover:text-[var(--axon-accent-blue)]'
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
                <span className="inline-flex min-w-[14px] items-center justify-center rounded-full border border-[rgba(175,215,255,0.35)] bg-[rgba(175,215,255,0.12)] px-1 text-[10px] text-[var(--axon-accent-pink-strong)]">
                  {activeOptionCount}
                </span>
              )}
            </button>
            <div className="h-[22px] w-px shrink-0 bg-[rgba(255,135,175,0.12)]" />
          </>
        )}

        {/* Action label — click to execute */}
        <button
          type="button"
          onClick={isProcessing ? cancel : execute}
          disabled={!isProcessing && !input.trim() && !NO_INPUT_MODES.has(mode)}
          className={`flex shrink-0 items-center gap-1.5 bg-transparent px-3 py-2.5 text-[11px] font-semibold uppercase tracking-wider transition-all duration-150 ${
            modeAppliedLabel
              ? 'text-[var(--axon-accent-pink)] drop-shadow-[0_0_6px_rgba(175,215,255,0.45)]'
              : 'text-[var(--axon-accent-blue)] hover:text-white'
          } disabled:opacity-40 disabled:hover:text-[var(--axon-accent-blue)]`}
          title={isProcessing ? 'Cancel' : 'Execute'}
        >
          {isProcessing ? (
            <>
              <span className="inline-block size-2.5 animate-spin rounded-full border-[1.5px] border-[rgba(175,215,255,0.2)] border-t-[var(--axon-accent-pink)]" />
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
          className="flex shrink-0 items-center justify-center rounded-r-[10px] bg-transparent px-3 py-2.5 text-[var(--axon-text-muted)] transition-colors duration-150 hover:text-[var(--axon-accent-blue)]"
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
          className={`absolute left-0 right-0 top-[calc(100%+6px)] z-50 space-y-1 rounded-xl border border-[rgba(255,135,175,0.15)] p-2 shadow-[0_16px_48px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,135,175,0.08)] backdrop-blur-xl transition-all duration-200 ${
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
                <div className="px-2.5 pb-1 pt-1.5 text-[10px] font-bold uppercase tracking-[0.15em] text-[var(--axon-text-dim)]">
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
                          ? 'bg-[rgba(175,215,255,0.12)] text-[var(--axon-accent-pink)]'
                          : 'text-[var(--axon-text-muted)] hover:bg-[rgba(255,135,175,0.1)] hover:text-[var(--axon-accent-blue)]'
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
            className={`absolute right-0 top-[calc(100%+6px)] z-50 w-[min(560px,92vw)] rounded-xl border border-[rgba(255,135,175,0.15)] p-2 shadow-[0_16px_48px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,135,175,0.08)] backdrop-blur-xl transition-all duration-200 ${
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
      {activeSuggestions.length > 0 && mentionKind !== 'none' && (
        <div className="rounded-lg border border-[rgba(255,135,175,0.14)] bg-[rgba(10,18,35,0.45)] px-2 py-1.5">
          <div className="mb-1 flex items-center justify-between text-[10px] uppercase tracking-wider text-[var(--axon-text-dim)]">
            <span>{mentionKind === 'mode' ? 'Mode Select' : 'File Context'}</span>
            <span className="text-[var(--axon-text-dim)]">{omniboxPhase.replace('-', ' ')}</span>
          </div>
          <div className="flex flex-wrap gap-1.5">
            {mentionKind === 'mode'
              ? mentionSuggestions.map((candidate, idx) => (
                  <button
                    key={candidate.id}
                    type="button"
                    onClick={() => {
                      setMentionSelectionIndex(idx)
                      void applyModeMentionCandidate(candidate)
                    }}
                    className={`rounded-md border px-2 py-1 text-[11px] transition-all ${
                      idx === mentionSelectionIndex
                        ? 'border-[rgba(175,215,255,0.5)] bg-[rgba(175,215,255,0.18)] text-[var(--axon-accent-pink-strong)]'
                        : 'border-[rgba(255,135,175,0.25)] bg-[rgba(255,135,175,0.08)] text-[var(--axon-accent-blue)] hover:bg-[rgba(255,135,175,0.14)]'
                    }`}
                  >
                    @{candidate.id}
                  </button>
                ))
              : fileSuggestions.map((candidate, idx) => (
                  <button
                    key={candidate.id}
                    type="button"
                    onClick={() => {
                      setMentionSelectionIndex(idx)
                      void applyFileMentionCandidate(candidate)
                    }}
                    className={`rounded-md border px-2 py-1 text-[11px] transition-all ${
                      idx === mentionSelectionIndex
                        ? 'border-[rgba(175,215,255,0.5)] bg-[rgba(175,215,255,0.18)] text-[var(--axon-accent-pink-strong)]'
                        : 'border-[rgba(255,135,175,0.25)] bg-[rgba(255,135,175,0.08)] text-[var(--axon-accent-blue)] hover:bg-[rgba(255,135,175,0.14)]'
                    }`}
                  >
                    @{candidate.label}
                  </button>
                ))}
          </div>
          <div className="mt-1 text-[10px] text-[var(--axon-text-dim)]">
            Tab/Enter apply · ↑/↓ change
          </div>
        </div>
      )}
      {Object.keys(fileContextMentions).length > 0 && (
        <div className="rounded-lg border border-[rgba(95,135,175,0.35)] bg-[rgba(30,41,59,0.35)] px-2 py-1.5">
          <div className="mb-1 text-[10px] uppercase tracking-wider text-[var(--axon-text-dim)]">
            Attached Context
          </div>
          <div className="flex flex-wrap gap-1.5">
            {Object.entries(fileContextMentions).map(([label]) => (
              <button
                key={label}
                type="button"
                onClick={() => {
                  setFileContextMentions((prev) => {
                    const next = { ...prev }
                    delete next[label]
                    return next
                  })
                }}
                className="rounded-md border border-[rgba(95,135,175,0.45)] bg-[rgba(95,135,175,0.12)] px-2 py-1 text-[11px] text-[var(--axon-accent-blue)]"
              >
                @{label} ×
              </button>
            ))}
          </div>
        </div>
      )}
      {modeAppliedLabel && (
        <div className="rounded-md border border-[rgba(175,215,255,0.35)] bg-[rgba(175,215,255,0.1)] px-2 py-1 text-[11px] text-[var(--axon-accent-pink-strong)]">
          Mode selected:{' '}
          <span className="font-semibold text-[var(--axon-accent-pink)]">{modeAppliedLabel}</span>
        </div>
      )}
    </div>
  )
}
