'use client'

import { SendHorizontal, Shield, ShieldCheck, ShieldOff, Square, Wrench } from 'lucide-react'
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
  MODE_CATEGORY_LABELS,
  MODE_CATEGORY_ORDER,
  MODES,
  type ModeId,
  NO_INPUT_MODES,
} from '@/lib/ws-protocol'
import { CommandOptionsPanel, type CommandOptionValues } from './command-options-panel'

export function shouldPreservePulseWorkspaceForMode(
  workspaceMode: string | null,
  execMode: ModeId,
): boolean {
  return (
    workspaceMode === 'pulse' &&
    (execMode === 'scrape' || execMode === 'crawl' || execMode === 'extract')
  )
}

export function Omnibox() {
  const { send, subscribe } = useAxonWs()
  const {
    startExecution,
    activateWorkspace,
    submitWorkspacePrompt,
    currentJobId,
    currentMode,
    workspaceMode,
    workspaceContext,
    pulseModel,
    pulsePermissionLevel,
    setPulseModel,
    setPulsePermissionLevel,
  } = useWsMessages()
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
  const [showModeSelector, setShowModeSelector] = useState(false)
  const [toolsOpen, setToolsOpen] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const omniboxRef = useRef<HTMLDivElement>(null)
  const toolsRef = useRef<HTMLDivElement>(null)
  const startTimeRef = useRef(0)
  const execIdRef = useRef(0)
  const [optionValues, setOptionValues] = useState<CommandOptionValues>({})

  function isUrlLikeToken(token: string): boolean {
    if (!token) return false
    if (/^https?:\/\//i.test(token)) return true
    if (token.includes('@')) return false
    return /^[a-z0-9.-]+\.[a-z]{2,}(?:[/:?#].*)?$/i.test(token)
  }

  function shouldRunCommandForInput(selectedMode: ModeId, rawInput: string): boolean {
    const trimmed = rawInput.trim()
    if (!trimmed) return NO_INPUT_MODES.has(selectedMode)
    const firstToken = trimmed.split(/\s+/)[0] ?? ''
    return isUrlLikeToken(firstToken)
  }

  function normalizeUrlInput(rawInput: string): string {
    const trimmed = rawInput.trim()
    const firstToken = trimmed.split(/\s+/)[0] ?? ''
    if (!trimmed || /^https?:\/\//i.test(firstToken)) return trimmed
    if (!isUrlLikeToken(firstToken)) return trimmed
    if (firstToken !== trimmed) return trimmed
    return `https://${trimmed}`
  }

  const selectedModeDef = MODES.find((m) => m.id === mode) ?? MODES[0]
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
  const effectiveDropdownOpen = dropdownOpen || mentionKind === 'mode'
  const willRunAsCommand = useMemo(() => shouldRunCommandForInput(mode, input), [mode, input])
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
  const contextFileCount = Object.keys(fileContextMentions).length
  const contextUtilizationPercent = useMemo(() => {
    if (!workspaceContext || workspaceContext.contextBudgetChars <= 0) return 0
    const ratio = (workspaceContext.contextCharsTotal / workspaceContext.contextBudgetChars) * 100
    if (ratio <= 0) return 0
    return Math.min(100, ratio)
  }, [workspaceContext])
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
        setToolsOpen(false)
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

  // Global "/" and Cmd/Ctrl+K shortcuts to focus the omnibox.
  useEffect(() => {
    function isEditableElement(target: EventTarget | null): boolean {
      if (!(target instanceof HTMLElement)) return false
      if (target.isContentEditable) return true
      const tag = target.tagName.toLowerCase()
      return tag === 'input' || tag === 'textarea' || tag === 'select'
    }

    function onKeyDown(event: KeyboardEvent) {
      const slashShortcut =
        event.key === '/' && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey
      const commandPaletteShortcut =
        (event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === 'k'
      if (!slashShortcut && !commandPaletteShortcut) return
      if (slashShortcut && isEditableElement(event.target)) return
      event.preventDefault()
      inputRef.current?.focus()
    }

    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [])

  useEffect(() => {
    const media = window.matchMedia('(min-width: 768px)')
    const update = () => setShowModeSelector(media.matches)
    update()
    media.addEventListener('change', update)
    return () => media.removeEventListener('change', update)
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
      const shouldRunCommand = shouldRunCommandForInput(execMode, trimmedInput)
      if (!shouldRunCommand) {
        activateWorkspace('pulse')
        if (trimmedInput) submitWorkspacePrompt(trimmedInput)
        return
      }

      const normalizedInput = normalizeUrlInput(trimmedInput)
      const { enrichedInput, contextFileLabels } = await buildInputWithFileContext(normalizedInput)

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

      const preservePulseWorkspace = shouldPreservePulseWorkspaceForMode(workspaceMode, execMode)
      startExecution(execMode, enrichedInput, { preserveWorkspace: preservePulseWorkspace })
    },
    [
      isProcessing,
      buildInputWithFileContext,
      activateWorkspace,
      workspaceMode,
      submitWorkspacePrompt,
      send,
      startExecution,
      optionValues,
    ],
  )

  const execute = useCallback(() => {
    const hasTypedInput = input.trim().length > 0
    void executeCommand(mode, input)
    if (hasTypedInput) setInput('')
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
      if (mentionKind === 'mode') {
        setInput('')
        setMentionSuggestions([])
        setFileSuggestions([])
        setMentionSelectionIndex(0)
        setModeAppliedLabel(MODES.find((m) => m.id === id)?.label ?? null)
      }
      if (NO_INPUT_MODES.has(id)) {
        setTimeout(() => {
          void executeCommand(id, '')
        }, 0)
      } else {
        inputRef.current?.focus()
      }
    },
    [executeCommand, mentionKind],
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
        if ((e.metaKey || e.ctrlKey) && !e.altKey) {
          e.preventDefault()
          execute()
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
        if (mentionKind === 'mode') {
          setInput('')
        }
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
        } min-h-[46px] focus-within:border-[rgba(255,135,175,0.4)] focus-within:shadow-[0_0_0_3px_rgba(255,135,175,0.08)]`}
        style={{
          background: 'rgba(10, 18, 35, 0.65)',
          borderWidth: '1.5px',
          borderStyle: 'solid',
          borderColor: isProcessing ? 'rgba(175,215,255, 0.4)' : 'rgba(255,135,175, 0.18)',
        }}
      >
        {/* Text input */}
        <input
          id="axon-omnibox-input"
          name="axon_omnibox_input"
          ref={inputRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={'@mention a tool or just start talking'}
          className="min-w-0 flex-1 bg-transparent px-3 py-2.5 font-mono text-[length:var(--text-base)] leading-[var(--leading-tight)] text-foreground outline-none placeholder:text-[var(--axon-text-subtle)] sm:px-4 sm:py-3"
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
          <span className="font-mono text-[length:var(--text-xs)] tracking-wide text-[var(--axon-text-muted)]">
            {statusText}
          </span>
        </div>

        {/* Divider */}
        <div className="h-[22px] w-px shrink-0 bg-[rgba(255,135,175,0.12)]" />

        {!NO_INPUT_MODES.has(mode) && input.trim().length > 0 && willRunAsCommand && (
          <>
            <div className="inline-flex shrink-0 items-center px-2.5">
              <span
                className={`ui-chip rounded-full border px-1.5 py-0.5 ${'border-[rgba(175,215,255,0.38)] bg-[rgba(175,215,255,0.12)] text-[var(--axon-accent-pink-strong)]'}`}
              >
                {selectedModeDef.label}
              </span>
            </div>
            <div className="h-[22px] w-px shrink-0 bg-[rgba(255,135,175,0.12)]" />
          </>
        )}

        {showModeSelector && (
          <>
            <div className="h-[22px] w-px shrink-0 bg-[rgba(255,135,175,0.12)]" />
            <div className="inline-flex shrink-0 items-center px-2.5">
              <span className="ui-chip rounded-full border border-[rgba(95,135,175,0.28)] bg-[rgba(10,18,35,0.48)] px-1.5 py-0.5 text-[var(--axon-accent-blue)]">
                {selectedModeDef.label}
              </span>
            </div>
          </>
        )}

        {workspaceMode === 'pulse' && (
          <>
            <div className="h-[22px] w-px shrink-0 bg-[rgba(255,135,175,0.12)]" />
            <div ref={toolsRef} className="relative flex shrink-0 items-center">
              <button
                type="button"
                onClick={() => {
                  setToolsOpen((prev) => !prev)
                  setDropdownOpen(false)
                  setOptionsOpen(false)
                }}
                className={`relative flex items-center justify-center rounded-md border px-2.5 py-2 text-[var(--axon-text-muted)] transition-colors duration-150 hover:text-[var(--axon-accent-blue)] ${
                  toolsOpen
                    ? 'border-[rgba(175,215,255,0.42)] bg-[rgba(175,215,255,0.12)]'
                    : 'border-[rgba(255,135,175,0.22)] bg-transparent'
                }`}
                title={`Pulse tools · ${pulseModel} · ${pulsePermissionLevel}`}
                aria-label="Pulse tools"
              >
                {pulsePermissionLevel === 'plan' ? (
                  <Shield className="size-3.5" />
                ) : pulsePermissionLevel === 'bypass-permissions' ? (
                  <ShieldOff className="size-3.5" />
                ) : (
                  <ShieldCheck className="size-3.5" />
                )}
                {isProcessing && currentMode && (
                  <span className="pointer-events-none absolute -right-0.5 -top-0.5 inline-flex size-2 animate-pulse rounded-full bg-[var(--axon-accent-pink)]" />
                )}
              </button>
              {toolsOpen && (
                <div
                  className="absolute right-0 top-[calc(100%+6px)] z-50 w-44 space-y-2 rounded-md border border-[rgba(255,135,175,0.2)] bg-[rgba(10,18,35,0.96)] p-2 shadow-[0_8px_24px_rgba(0,0,0,0.45)]"
                  role="dialog"
                  aria-label="Pulse model and permission controls"
                >
                  <div className="flex items-center gap-1.5 text-[var(--axon-text-dim)]">
                    <Wrench className="size-3" />
                    <span className="ui-label">Tools</span>
                  </div>
                  <label className="block space-y-1">
                    <span className="ui-label">Model</span>
                    <select
                      id="omnibox-pulse-model-selector"
                      name="omnibox_pulse_model_selector"
                      value={pulseModel}
                      onChange={(e) => setPulseModel(e.target.value as typeof pulseModel)}
                      className="h-7 w-full rounded border border-[rgba(95,135,175,0.24)] bg-[rgba(10,18,35,0.72)] px-2 text-[length:var(--text-xs)] font-semibold uppercase tracking-[0.04em] text-[var(--axon-text-primary)] outline-none"
                      aria-label="Model selector"
                    >
                      <option value="sonnet">Sonnet</option>
                      <option value="opus">Opus</option>
                      <option value="haiku">Haiku</option>
                    </select>
                  </label>
                  <label className="block space-y-1">
                    <span className="ui-label">Permission</span>
                    <select
                      id="omnibox-pulse-permission-selector"
                      name="omnibox_pulse_permission_selector"
                      value={pulsePermissionLevel}
                      onChange={(e) =>
                        setPulsePermissionLevel(e.target.value as typeof pulsePermissionLevel)
                      }
                      className="h-7 w-full rounded border border-[rgba(255,135,175,0.2)] bg-[rgba(10,18,35,0.72)] px-2 text-[length:var(--text-xs)] font-semibold uppercase tracking-[0.04em] text-[var(--axon-text-primary)] outline-none"
                      aria-label="Permission selector"
                    >
                      <option value="plan">Plan</option>
                      <option value="accept-edits">Accept</option>
                      <option value="bypass-permissions">Bypass</option>
                    </select>
                  </label>
                </div>
              )}
            </div>
          </>
        )}

        {/* Options button */}
        {hasOptions && (
          <>
            <button
              type="button"
              onClick={() => {
                setOptionsOpen((prev) => !prev)
                setDropdownOpen(false)
              }}
              className={`flex shrink-0 items-center gap-1.5 bg-transparent px-2.5 py-2.5 text-[length:var(--text-xs)] font-semibold uppercase tracking-wider transition-colors duration-150 ${
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
                <span className="inline-flex min-w-[14px] items-center justify-center rounded-full border border-[rgba(175,215,255,0.35)] bg-[rgba(175,215,255,0.12)] px-1 text-[length:var(--text-2xs)] leading-[var(--leading-tight)] text-[var(--axon-accent-pink-strong)]">
                  {activeOptionCount}
                </span>
              )}
            </button>
            <div className="h-[22px] w-px shrink-0 bg-[rgba(255,135,175,0.12)]" />
          </>
        )}

        {/* Action control — icon-only send/cancel */}
        <button
          type="button"
          onClick={isProcessing ? cancel : execute}
          disabled={!isProcessing && !input.trim() && !NO_INPUT_MODES.has(mode)}
          className={`flex shrink-0 items-center gap-1.5 bg-transparent px-3 py-2.5 text-[length:var(--text-xs)] font-semibold uppercase tracking-wider transition-all duration-150 ${
            modeAppliedLabel
              ? 'text-[var(--axon-accent-pink)] drop-shadow-[0_0_6px_rgba(175,215,255,0.45)]'
              : 'text-[var(--axon-accent-blue)] hover:text-white'
          } disabled:opacity-40 disabled:hover:text-[var(--axon-accent-blue)]`}
          title={isProcessing ? 'Cancel' : 'Execute'}
        >
          {isProcessing ? <Square className="size-3.5" /> : <SendHorizontal className="size-3.5" />}
        </button>

        {showModeSelector && (
          /* Arrow toggle — click to open mode dropdown */
          <button
            type="button"
            onClick={() => setDropdownOpen((prev) => !prev)}
            className="flex shrink-0 items-center justify-center rounded-r-[10px] bg-transparent px-3 py-2.5 text-[var(--axon-text-muted)] transition-colors duration-150 hover:text-[var(--axon-accent-blue)]"
            title="Select mode"
          >
            <svg
              className={`size-3.5 transition-transform duration-200 ${effectiveDropdownOpen ? 'rotate-90' : ''}`}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
            </svg>
          </button>
        )}

        {/* Mode dropdown — triggered by arrow button OR by typing @ */}
        <div
          className={`absolute left-0 right-0 top-[calc(100%+6px)] z-50 max-h-[65vh] space-y-1 overflow-y-auto rounded-xl border border-[rgba(255,135,175,0.15)] p-2 shadow-[0_16px_48px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,135,175,0.08)] backdrop-blur-xl transition-all duration-200 ${
            effectiveDropdownOpen
              ? 'visible translate-y-0 opacity-100'
              : 'invisible -translate-y-1 opacity-0'
          }`}
          style={{ background: 'rgba(15, 23, 42, 0.95)' }}
        >
          {mentionKind === 'mode' && activeMentionToken?.query && (
            <div className="px-2.5 pb-1 pt-1 text-[length:var(--text-2xs)] text-[var(--axon-text-dim)]">
              Showing results for{' '}
              <span className="text-[var(--axon-accent-blue)]">@{activeMentionToken.query}</span>
            </div>
          )}
          {MODE_CATEGORY_ORDER.map((cat) => {
            const items = groupedModes.get(cat)
            if (!items || items.length === 0) return null
            const visibleItems =
              mentionKind === 'mode' && activeMentionToken?.query
                ? items.filter(
                    (m) =>
                      m.id.includes(activeMentionToken.query.toLowerCase()) ||
                      m.label.toLowerCase().includes(activeMentionToken.query.toLowerCase()),
                  )
                : items
            if (visibleItems.length === 0) return null
            return (
              <div key={cat}>
                <div className="px-2.5 pb-1 pt-1.5 text-[length:var(--text-2xs)] font-bold uppercase tracking-[0.15em] text-[var(--axon-text-dim)]">
                  {MODE_CATEGORY_LABELS[cat]}
                </div>
                <div className="grid grid-cols-[repeat(auto-fill,minmax(118px,1fr))] gap-0.5">
                  {visibleItems.map((m) => (
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
      {fileSuggestions.length > 0 && mentionKind === 'file' && (
        <div className="rounded-lg border border-[rgba(255,135,175,0.14)] bg-[rgba(10,18,35,0.45)] px-2 py-1.5">
          <div className="ui-label mb-1 flex items-center justify-between">
            <span>File Context</span>
            <span className="text-[var(--axon-text-dim)]">{omniboxPhase.replace('-', ' ')}</span>
          </div>
          <div className="flex flex-wrap gap-1.5">
            {fileSuggestions.map((candidate, idx) => (
              <button
                key={candidate.id}
                type="button"
                onClick={() => {
                  setMentionSelectionIndex(idx)
                  void applyFileMentionCandidate(candidate)
                }}
                className={`rounded-md border px-2 py-1 text-[length:var(--text-xs)] font-semibold transition-all ${
                  idx === mentionSelectionIndex
                    ? 'border-[rgba(175,215,255,0.5)] bg-[rgba(175,215,255,0.18)] text-[var(--axon-accent-pink-strong)]'
                    : 'border-[rgba(255,135,175,0.25)] bg-[rgba(255,135,175,0.08)] text-[var(--axon-accent-blue)] hover:bg-[rgba(255,135,175,0.14)]'
                }`}
              >
                @{candidate.label}
              </button>
            ))}
          </div>
          <div className="ui-meta mt-1">Tab/Enter apply · ↑/↓ change</div>
        </div>
      )}
      {Object.keys(fileContextMentions).length > 0 && (
        <div className="rounded-lg border border-[rgba(95,135,175,0.35)] bg-[rgba(30,41,59,0.35)] px-2 py-1.5">
          <div className="ui-label mb-1">Attached Context</div>
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
                className="rounded-md border border-[rgba(95,135,175,0.45)] bg-[rgba(95,135,175,0.12)] px-2 py-1 text-[length:var(--text-xs)] font-semibold text-[var(--axon-accent-blue)]"
              >
                @{label} ×
              </button>
            ))}
          </div>
        </div>
      )}
      {workspaceMode === 'pulse' && workspaceContext && workspaceContext.turns > 0 && (
        <div
          className={`rounded-md border border-[rgba(95,135,175,0.2)] bg-[rgba(10,18,35,0.32)] px-2 py-1.5 ${
            isProcessing ? 'shadow-[0_0_0_1px_rgba(175,215,255,0.2)]' : ''
          }`}
          title={`${workspaceContext.turns} turns · ${workspaceContext.threadSourceCount} active sources · ${contextFileCount} files · ${workspaceContext.contextCharsTotal.toLocaleString()} / ${workspaceContext.contextBudgetChars.toLocaleString()} chars · last ${(workspaceContext.lastLatencyMs / 1000).toFixed(1)}s${
            isProcessing && currentMode ? ` · processing ${currentMode}` : ''
          }`}
        >
          <div className="h-1.5 overflow-hidden rounded-full bg-[rgba(255,135,175,0.12)]">
            <div
              className={`h-full rounded-full bg-[linear-gradient(90deg,rgba(95,135,175,0.85),rgba(255,135,175,0.9)) ${
                isProcessing ? 'animate-pulse' : ''
              }`}
              style={{ width: `${contextUtilizationPercent}%` }}
            />
          </div>
        </div>
      )}
      {modeAppliedLabel && (
        <div className="rounded-md border border-[rgba(175,215,255,0.35)] bg-[rgba(175,215,255,0.1)] px-2 py-1 text-[length:var(--text-xs)] text-[var(--axon-accent-pink-strong)]">
          Mode selected:{' '}
          <span className="font-semibold text-[var(--axon-accent-pink)]">{modeAppliedLabel}</span>
        </div>
      )}
      <div className="ui-meta rounded-md border border-[rgba(95,135,175,0.16)] bg-[rgba(10,18,35,0.28)] px-2 py-1">
        Enter send · @mode switch · Alt+1/2/3 model · Alt+Shift+1/2/3 permission
      </div>
    </div>
  )
}
