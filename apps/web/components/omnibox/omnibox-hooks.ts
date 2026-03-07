'use client'

import { useCallback, useMemo, useRef, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { getCommandSpec } from '@/lib/axon-command-map'
import { deriveOmniboxPhase } from '@/lib/omnibox'
import type { ModeCategory, ModeDefinition, ModeId } from '@/lib/ws-protocol'
import { MODE_CATEGORY_ORDER, MODES, NO_INPUT_MODES } from '@/lib/ws-protocol'
import { useOmniboxExecution } from './hooks/use-omnibox-execution'
import { useOmniboxKeyboard } from './hooks/use-omnibox-keyboard'
import { useOmniboxMentions } from './hooks/use-omnibox-mentions'
import { useOmniboxEffects } from './omnibox-effects'
import { shouldRunCommandForInput } from './utils'

const MENTION_TIP_SEEN_KEY = 'axon.web.mention-tip-seen'

export function useOmniboxState() {
  const { subscribe } = useAxonWs()

  // ── Core state ──────────────────────────────────────────────────────
  const [mode, setMode] = useState<ModeId>('scrape')
  const [input, setInput] = useState('')
  const [dropdownOpen, setDropdownOpen] = useState(false)
  const [optionsOpen, setOptionsOpen] = useState(false)
  const [showModeSelector, setShowModeSelector] = useState(false)
  const [toolsOpen, setToolsOpen] = useState(false)
  const [placeholderIdx, setPlaceholderIdx] = useState(0)
  const [placeholderVisible, setPlaceholderVisible] = useState(true)
  const [isFocused, setIsFocused] = useState(false)
  const [mentionTipSeen, setMentionTipSeen] = useState(() => {
    if (typeof window === 'undefined') return true
    return localStorage.getItem(MENTION_TIP_SEEN_KEY) === '1'
  })

  // ── Refs ────────────────────────────────────────────────────────────
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const omniboxRef = useRef<HTMLDivElement>(null)
  const toolsRef = useRef<HTMLDivElement>(null)

  // ── Sub-hooks ──────────────────────────────────────────────────────
  const mentions = useOmniboxMentions({ input, setInput })

  const execution = useOmniboxExecution({
    mode,
    input,
    setInput,
    buildInputWithFileContext: mentions.buildInputWithFileContext,
  })

  // ── Bridge callbacks (touch both mentions and execution) ──────────
  const selectMode = useCallback(
    (id: ModeId) => {
      setMode(id)
      setDropdownOpen(false)
      setOptionsOpen(false)
      execution.setOptionValues({})
      if (mentions.mentionKind === 'mode') {
        setInput('')
        mentions.setMentionSuggestions([])
        mentions.setFileSuggestions([])
        mentions.setMentionSelectionIndex(0)
        mentions.setModeAppliedLabel(MODES.find((m) => m.id === id)?.label ?? null)
      }
      if (NO_INPUT_MODES.has(id)) {
        setTimeout(() => {
          void execution.executeCommand(id, '')
        }, 0)
      } else {
        inputRef.current?.focus()
      }
    },
    [
      execution.setOptionValues,
      execution.executeCommand,
      mentions.mentionKind,
      mentions.setMentionSuggestions,
      mentions.setFileSuggestions,
      mentions.setMentionSelectionIndex,
      mentions.setModeAppliedLabel,
    ],
  )

  const applyModeMentionCandidate = useCallback(
    (candidate: ModeDefinition) => {
      selectMode(candidate.id as ModeId)
      setInput('')
      mentions.setMentionSuggestions([])
      mentions.setFileSuggestions([])
      mentions.setMentionSelectionIndex(0)
      mentions.setModeAppliedLabel(candidate.label)
      return true
    },
    [
      selectMode,
      mentions.setMentionSuggestions,
      mentions.setFileSuggestions,
      mentions.setMentionSelectionIndex,
      mentions.setModeAppliedLabel,
    ],
  )

  const applyActiveSuggestion = useCallback(() => {
    if (mentions.mentionKind === 'mode') {
      const selected = mentions.mentionSuggestions[mentions.mentionSelectionIndex]
      return selected ? applyModeMentionCandidate(selected) : false
    }
    const selected = mentions.fileSuggestions[mentions.mentionSelectionIndex]
    return selected ? mentions.applyFileMentionCandidate(selected) : false
  }, [
    mentions.mentionSelectionIndex,
    mentions.mentionKind,
    mentions.mentionSuggestions,
    mentions.fileSuggestions,
    applyModeMentionCandidate,
    mentions.applyFileMentionCandidate,
  ])

  const { handleKeyDown } = useOmniboxKeyboard({
    activeSuggestions: mentions.activeSuggestions,
    mentionKind: mentions.mentionKind,
    applyActiveSuggestion,
    execute: execution.execute,
    setDropdownOpen,
    setOptionsOpen,
    setMentionSuggestions: mentions.setMentionSuggestions,
    setFileSuggestions: mentions.setFileSuggestions,
    setMentionSelectionIndex: mentions.setMentionSelectionIndex,
    setInput,
  })

  // ── Derived values ──────────────────────────────────────────────────
  const selectedModeDef = MODES.find((m) => m.id === mode) ?? MODES[0]
  const hasOptions = (getCommandSpec(mode)?.commandOptions.length ?? 0) > 0
  const activeOptionCount = useMemo(
    () => Object.values(execution.optionValues).filter((val) => val !== '' && val !== false).length,
    [execution.optionValues],
  )
  const effectiveDropdownOpen = dropdownOpen || mentions.mentionKind === 'mode'
  const willRunAsCommand = useMemo(() => shouldRunCommandForInput(mode, input), [mode, input])
  const omniboxPhase = useMemo(
    () =>
      deriveOmniboxPhase({
        isProcessing: execution.isProcessing,
        input,
        mentionKind: mentions.mentionKind,
        hasModeFeedback: Boolean(mentions.modeAppliedLabel),
      }),
    [input, execution.isProcessing, mentions.mentionKind, mentions.modeAppliedLabel],
  )
  const contextUtilizationPercent = useMemo(() => {
    if (!execution.workspaceContext || execution.workspaceContext.contextBudgetChars <= 0) return 0
    const ratio =
      (execution.workspaceContext.contextCharsTotal /
        execution.workspaceContext.contextBudgetChars) *
      100
    if (ratio <= 0) return 0
    return Math.min(100, ratio)
  }, [execution.workspaceContext])
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

  // ── Effects ─────────────────────────────────────────────────────────
  useOmniboxEffects({
    mode,
    input,
    isProcessing: execution.isProcessing,
    isFocused,
    statusText: execution.statusText,
    statusType: execution.statusType,
    modeAppliedLabel: mentions.modeAppliedLabel,
    activeMentionToken: mentions.activeMentionToken,
    mentionKind: mentions.mentionKind,
    localDocFiles: mentions.localDocFiles,
    recentFileSelections: mentions.recentFileSelections,
    workspaceMode: execution.workspaceMode,
    inputRef,
    omniboxRef,
    setDropdownOpen,
    setOptionsOpen,
    setToolsOpen,
    setIsProcessing: execution.setIsProcessing,
    setStatusText: execution.setStatusText,
    setStatusType: execution.setStatusType,
    setCompletionStatus: execution.setCompletionStatus,
    setShowModeSelector,
    setLocalDocFiles: mentions.setLocalDocFiles,
    setMentionSuggestions: mentions.setMentionSuggestions,
    setFileSuggestions: mentions.setFileSuggestions,
    setMentionSelectionIndex: mentions.setMentionSelectionIndex,
    setModeAppliedLabel: mentions.setModeAppliedLabel,
    setPlaceholderVisible,
    setPlaceholderIdx,
    setInput,
    subscribe,
  })

  return {
    // State
    mode,
    input,
    isProcessing: execution.isProcessing,
    statusText: execution.statusText,
    statusType: execution.statusType,
    dropdownOpen,
    optionsOpen,
    mentionSuggestions: mentions.mentionSuggestions,
    fileSuggestions: mentions.fileSuggestions,
    mentionSelectionIndex: mentions.mentionSelectionIndex,
    modeAppliedLabel: mentions.modeAppliedLabel,
    fileContextMentions: mentions.fileContextMentions,
    showModeSelector,
    toolsOpen,
    optionValues: execution.optionValues,
    placeholderIdx,
    placeholderVisible,
    isFocused,
    completionStatus: execution.completionStatus,
    mentionTipSeen,

    // Derived
    activeMentionToken: mentions.activeMentionToken,
    selectedModeDef,
    hasOptions,
    activeOptionCount,
    mentionKind: mentions.mentionKind,
    activeSuggestions: mentions.activeSuggestions,
    effectiveDropdownOpen,
    willRunAsCommand,
    omniboxPhase,
    contextUtilizationPercent,
    groupedModes,

    // Workspace
    workspaceMode: execution.workspaceMode,
    workspaceContext: execution.workspaceContext,
    workspaceResumeSessionId: execution.workspaceResumeSessionId,
    pulseAgent: execution.pulseAgent,
    pulseModel: execution.pulseModel,
    pulsePermissionLevel: execution.pulsePermissionLevel,
    acpConfigOptions: execution.acpConfigOptions,
    currentMode: execution.currentMode,
    isProcessingWithCurrentMode: execution.isProcessing && Boolean(execution.currentMode),

    // Actions
    setInput,
    setDropdownOpen,
    setOptionsOpen,
    setToolsOpen,
    setMentionSelectionIndex: mentions.setMentionSelectionIndex,
    setIsFocused,
    setMentionTipSeen,
    setPulseAgent: execution.setPulseAgent,
    setPulseModel: execution.setPulseModel,
    setPulsePermissionLevel: execution.setPulsePermissionLevel,
    execute: execution.execute,
    cancel: execution.cancel,
    selectMode,
    applyActiveSuggestion,
    applyFileMentionCandidate: mentions.applyFileMentionCandidate,
    removeFileContextMention: mentions.removeFileContextMention,
    setOptionValues: execution.setOptionValues,
    handleKeyDown,

    // Refs
    inputRef,
    omniboxRef,
    toolsRef,
  }
}
