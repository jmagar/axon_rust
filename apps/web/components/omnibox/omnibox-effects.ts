'use client'

import { useEffect } from 'react'
import {
  type LocalDocFile,
  type MentionKind,
  rankFileSuggestions,
  rankModeSuggestions,
} from '@/lib/omnibox'
import type { CompletionStatus } from '@/lib/omnibox-types'
import type { ModeDefinition, ModeId, WsServerMsg } from '@/lib/ws-protocol'
import { MODES } from '@/lib/ws-protocol'
import { PLACEHOLDER_TEXTS } from './utils'

interface ActiveMentionToken {
  query: string
  start: number
  end: number
}

interface OmniboxEffectsParams {
  // State values
  mode: ModeId
  input: string
  isProcessing: boolean
  isFocused: boolean
  statusText: string
  statusType: 'processing' | 'done' | 'error'
  modeAppliedLabel: string | null
  activeMentionToken: ActiveMentionToken | null
  mentionKind: MentionKind
  localDocFiles: LocalDocFile[]
  recentFileSelections: Record<string, number>
  workspaceMode: string | null

  // Refs
  inputRef: React.RefObject<HTMLTextAreaElement | null>
  omniboxRef: React.RefObject<HTMLDivElement | null>

  // Setters
  setDropdownOpen: (value: boolean) => void
  setOptionsOpen: (value: boolean) => void
  setToolsOpen: (value: boolean) => void
  setIsProcessing: (value: boolean) => void
  setStatusText: (value: string) => void
  setStatusType: (value: 'processing' | 'done' | 'error') => void
  setCompletionStatus: (value: CompletionStatus | null) => void
  setShowModeSelector: (value: boolean) => void
  setLocalDocFiles: (value: LocalDocFile[]) => void
  setMentionSuggestions: (value: ModeDefinition[]) => void
  setFileSuggestions: (value: LocalDocFile[]) => void
  setMentionSelectionIndex: (value: number) => void
  setModeAppliedLabel: (value: string | null) => void
  setPlaceholderVisible: (value: boolean) => void
  setPlaceholderIdx: (value: number | ((prev: number) => number)) => void
  setInput: (value: string) => void

  // WS
  subscribe: (handler: (msg: WsServerMsg) => void) => () => void
}

export function useOmniboxEffects(params: OmniboxEffectsParams) {
  const {
    mode,
    input,
    isProcessing,
    isFocused,
    statusText,
    statusType,
    modeAppliedLabel,
    activeMentionToken,
    mentionKind,
    localDocFiles,
    recentFileSelections,
    workspaceMode,
    inputRef,
    omniboxRef,
    setDropdownOpen,
    setOptionsOpen,
    setToolsOpen,
    setIsProcessing,
    setStatusText,
    setStatusType,
    setCompletionStatus,
    setShowModeSelector,
    setLocalDocFiles,
    setMentionSuggestions,
    setFileSuggestions,
    setMentionSelectionIndex,
    setModeAppliedLabel,
    setPlaceholderVisible,
    setPlaceholderIdx,
    setInput,
    subscribe,
  } = params

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
  }, [omniboxRef, setDropdownOpen, setOptionsOpen, setToolsOpen])

  // Subscribe to WS for command completion updates
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
  }, [mode, subscribe, setIsProcessing, setStatusText, setStatusType])

  // Capture completion status so it persists after statusText clears
  useEffect(() => {
    if (statusType === 'done' || statusType === 'error') {
      setCompletionStatus({ type: statusType, text: statusText })
      const t = setTimeout(() => setCompletionStatus(null), 4000)
      return () => clearTimeout(t)
    }
  }, [statusType, statusText, setCompletionStatus])

  // Global "/" and Cmd/Ctrl+K shortcuts to focus the omnibox
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
  }, [inputRef])

  // Media query for mode selector visibility
  useEffect(() => {
    const media = window.matchMedia('(min-width: 768px)')
    const update = () => setShowModeSelector(media.matches)
    update()
    media.addEventListener('change', update)
    return () => media.removeEventListener('change', update)
  }, [setShowModeSelector])

  // Fetch local doc files
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
  }, [setLocalDocFiles])

  // Mention suggestions
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
  }, [
    activeMentionToken,
    localDocFiles,
    mentionKind,
    recentFileSelections,
    setMentionSuggestions,
    setFileSuggestions,
    setMentionSelectionIndex,
  ])

  // Mode applied label auto-clear
  useEffect(() => {
    if (!modeAppliedLabel) return
    const timer = setTimeout(() => setModeAppliedLabel(null), 900)
    return () => clearTimeout(timer)
  }, [modeAppliedLabel, setModeAppliedLabel])

  // Placeholder rotation
  useEffect(() => {
    if (input || isFocused || isProcessing) return
    let innerTimeout: ReturnType<typeof setTimeout> | undefined
    const interval = setInterval(() => {
      setPlaceholderVisible(false)
      innerTimeout = setTimeout(() => {
        setPlaceholderIdx((prev: number) => (prev + 1) % PLACEHOLDER_TEXTS.length)
        setPlaceholderVisible(true)
      }, 350)
    }, 3500)
    return () => {
      clearInterval(interval)
      clearTimeout(innerTimeout)
    }
  }, [input, isFocused, isProcessing, setPlaceholderVisible, setPlaceholderIdx])

  // Auto-resize textarea
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentional — input triggers the resize, scrollHeight is read from DOM not state
  useEffect(() => {
    const el = inputRef.current
    if (!el) return
    el.style.height = '1px'
    const capped = Math.min(el.scrollHeight, 160)
    el.style.height = `${capped}px`
    el.style.overflowY = el.scrollHeight > 160 ? 'auto' : 'hidden'
  }, [input])

  // Re-run height calc on container resize
  useEffect(() => {
    const el = inputRef.current
    const container = el?.parentElement
    if (!el || !container) return
    const observer = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        const previousHeight = el.style.height
        const previousOverflow = el.style.overflowY
        el.style.height = '1px'
        const capped = Math.min(el.scrollHeight, 160)
        const nextHeight = `${capped}px`
        const nextOverflow = el.scrollHeight > 160 ? 'auto' : 'hidden'
        if (previousHeight === nextHeight && previousOverflow === nextOverflow) {
          el.style.height = previousHeight
          el.style.overflowY = previousOverflow
          return
        }
        el.style.height = nextHeight
        el.style.overflowY = nextOverflow
      })
    })
    observer.observe(container)
    return () => observer.disconnect()
  }, [inputRef])

  // Clear input when navigating away from Pulse workspace
  useEffect(() => {
    if (workspaceMode !== 'pulse') {
      setInput('')
    }
  }, [workspaceMode, setInput])
}
