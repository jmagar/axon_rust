'use client'

import { usePathname } from 'next/navigation'
import type React from 'react'
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { getAcpModelConfigOption } from '@/lib/pulse/acp-config'
import { probePulseConfigOptions } from '@/lib/pulse/config-api'
import type { AcpConfigOption } from '@/lib/pulse/types'
import { PulseAgent, PulsePermissionLevel } from '@/lib/pulse/types'
import type { CrawlFile, WsLifecycleEntry, WsServerMsg } from '@/lib/ws-protocol'
import { handleWsMessage } from './ws-messages/handlers'
import { makeInitialRuntimeState, reduceRuntimeState } from './ws-messages/runtime'
import type {
  CancelResponseState,
  CrawlProgress,
  LogLine,
  PulseWorkspaceAgent,
  PulseWorkspaceModel,
  PulseWorkspacePermission,
  RecentRun,
  ScreenshotFile,
  WorkspaceContextState,
  WsMessagesActions,
  WsMessagesContextValue,
  WsMessagesExecutionState,
  WsMessagesWorkspaceState,
} from './ws-messages/types'

const WsMessagesContext = createContext<WsMessagesContextValue | null>(null)
const WsMessagesExecutionContext = createContext<WsMessagesExecutionState | null>(null)
const WsMessagesWorkspaceContext = createContext<WsMessagesWorkspaceState | null>(null)
const WsMessagesActionsContext = createContext<WsMessagesActions | null>(null)

function useRequiredContext<T>(context: React.Context<T | null>, errorMessage: string): T {
  const value = useContext(context)
  if (!value) throw new Error(errorMessage)
  return value
}

export function useWsMessages() {
  return useRequiredContext(
    WsMessagesContext,
    'useWsMessages must be used within WsMessagesProvider',
  )
}

export function useWsExecutionState() {
  return useRequiredContext(
    WsMessagesExecutionContext,
    'useWsExecutionState must be used within WsMessagesProvider',
  )
}

export function useWsWorkspaceState() {
  return useRequiredContext(
    WsMessagesWorkspaceContext,
    'useWsWorkspaceState must be used within WsMessagesProvider',
  )
}

export function useWsMessageActions() {
  return useRequiredContext(
    WsMessagesActionsContext,
    'useWsMessageActions must be used within WsMessagesProvider',
  )
}

export {
  WsMessagesActionsContext,
  WsMessagesContext,
  WsMessagesExecutionContext,
  WsMessagesWorkspaceContext,
  makeInitialRuntimeState,
  reduceRuntimeState,
}
export type {
  CancelResponseState,
  CrawlProgress,
  LogLine,
  PulseWorkspaceAgent,
  PulseWorkspaceModel,
  PulseWorkspacePermission,
  RecentRun,
  ScreenshotFile,
  WorkspaceContextState,
}

// ── localStorage helpers ────────────────────────────────────────────────────

const LS_WORKSPACE_MODE = 'axon.web.workspace-mode'
const LS_PULSE_AGENT = 'axon.web.pulse-agent'
const LS_PULSE_MODEL = 'axon.web.pulse-model'
const LS_PULSE_PERMISSION = 'axon.web.pulse-permission'

const VALID_AGENTS = new Set(PulseAgent.options)
const VALID_PERMISSIONS = new Set(PulsePermissionLevel.options)

function safeGetItem(key: string): string | null {
  try {
    return window.localStorage.getItem(key)
  } catch {
    return null
  }
}

function safeSetItem(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value)
  } catch {
    // Ignore storage errors (quota exceeded, private browsing, etc.)
  }
}

function safeRemoveItem(key: string): void {
  try {
    window.localStorage.removeItem(key)
  } catch {
    // Ignore storage errors.
  }
}

/**
 * Validate a raw localStorage string against a known set of allowed values.
 * Returns the validated value or the fallback if invalid/missing.
 */
function validateStoredEnum<T extends string>(
  raw: string | null,
  allowed: Set<string>,
  fallback: T,
): T {
  if (raw && allowed.has(raw)) return raw as T
  return fallback
}

// ── Provider hook ───────────────────────────────────────────────────────────

export function useWsMessagesProvider() {
  const pathname = usePathname()
  const { subscribe, send } = useAxonWs()
  const [markdownContent, setMarkdownContent] = useState('')
  const [logLines, setLogLines] = useState<LogLine[]>([])
  const [errorMessage, setErrorMessage] = useState('')
  const [recentRuns, setRecentRuns] = useState<RecentRun[]>([])
  const runIdCounter = useRef(0)
  const [isProcessing, setIsProcessing] = useState(false)
  const [hasResults, setHasResults] = useState(false)
  const [crawlFiles, setCrawlFiles] = useState<CrawlFile[]>([])
  const [selectedFile, setSelectedFile] = useState<string | null>(null)
  const [_virtualFileContentByPath, setVirtualFileContentByPath] = useState<Record<string, string>>(
    {},
  )
  const [_currentOutputDir, setCurrentOutputDir] = useState<string | null>(null)
  const currentModeRef = useRef('')
  const currentInputRef = useRef('')
  const [currentMode, setCurrentMode] = useState('')
  const [crawlProgress, setCrawlProgress] = useState<CrawlProgress | null>(null)
  const [stdoutLines, setStdoutLines] = useState<string[]>([])
  const [stdoutJson, setStdoutJson] = useState<unknown[]>([])
  const [commandMode, setCommandMode] = useState<string | null>(null)
  const [screenshotFiles, setScreenshotFiles] = useState<ScreenshotFile[]>([])
  const [currentJobId, setCurrentJobId] = useState<string | null>(null)
  const currentJobIdRef = useRef<string | null>(null)
  const [lifecycleEntries, setLifecycleEntries] = useState<WsLifecycleEntry[]>([])
  const [cancelResponse, setCancelResponse] = useState<CancelResponseState | null>(null)
  const [workspaceMode, setWorkspaceMode] = useState<string | null>('pulse')
  const [workspacePrompt, setWorkspacePrompt] = useState<string | null>(null)
  const [workspacePromptVersion, setWorkspacePromptVersion] = useState(0)
  const [workspaceResumeSessionId, setWorkspaceResumeSessionId] = useState<string | null>(null)
  const [workspaceResumeVersion, setWorkspaceResumeVersion] = useState(0)
  const [workspaceContext, setWorkspaceContext] = useState<WorkspaceContextState | null>(null)

  // ACP-related state — grouped together since they are logically coupled
  // and frequently updated as a set (agent change triggers config probe,
  // config probe updates options, options influence model selection).
  const [pulseAgent, setPulseAgent] = useState<PulseWorkspaceAgent>('claude')
  const [pulseModel, setPulseModel] = useState<PulseWorkspaceModel>('sonnet')
  const [pulsePermissionLevel, setPulsePermissionLevel] =
    useState<PulseWorkspacePermission>('accept-edits')
  const [acpConfigOptions, setAcpConfigOptions] = useState<AcpConfigOption[]>([])

  const selectedFileRef = useRef<string | null>(null)
  const crawlFilesRef = useRef<CrawlFile[]>([])
  const stdoutJsonRef = useRef<unknown[]>([])
  const currentOutputDirRef = useRef<string | null>(null)
  const virtualFileContentByPathRef = useRef<Record<string, string>>({})

  const setCrawlFilesTracked = useCallback((action: React.SetStateAction<CrawlFile[]>) => {
    if (typeof action === 'function') {
      setCrawlFiles((prev) => {
        const next = action(prev)
        crawlFilesRef.current = next
        return next
      })
    } else {
      crawlFilesRef.current = action
      setCrawlFiles(action)
    }
  }, [])

  const setSelectedFileTracked = useCallback((action: React.SetStateAction<string | null>) => {
    if (typeof action === 'function') {
      setSelectedFile((prev) => {
        const next = action(prev)
        selectedFileRef.current = next
        return next
      })
    } else {
      selectedFileRef.current = action
      setSelectedFile(action)
    }
  }, [])

  const setStdoutJsonTracked = useCallback((action: React.SetStateAction<unknown[]>) => {
    if (typeof action === 'function') {
      setStdoutJson((prev) => {
        const next = action(prev)
        stdoutJsonRef.current = next
        return next
      })
    } else {
      stdoutJsonRef.current = action
      setStdoutJson(action)
    }
  }, [])

  const setCurrentOutputDirTracked = useCallback((action: React.SetStateAction<string | null>) => {
    if (typeof action === 'function') {
      setCurrentOutputDir((prev) => {
        const next = action(prev)
        currentOutputDirRef.current = next
        return next
      })
    } else {
      currentOutputDirRef.current = action
      setCurrentOutputDir(action)
    }
  }, [])

  const setVirtualFileContentByPathTracked = useCallback(
    (action: React.SetStateAction<Record<string, string>>) => {
      if (typeof action === 'function') {
        setVirtualFileContentByPath((prev) => {
          const next = action(prev)
          virtualFileContentByPathRef.current = next
          return next
        })
      } else {
        virtualFileContentByPathRef.current = action
        setVirtualFileContentByPath(action)
      }
    },
    [],
  )

  // ── localStorage: read on mount (once) ──────────────────────────────────

  useEffect(() => {
    const storedMode = safeGetItem(LS_WORKSPACE_MODE)
    if (storedMode) setWorkspaceMode(storedMode)

    const storedAgent = validateStoredEnum(
      safeGetItem(LS_PULSE_AGENT),
      VALID_AGENTS,
      'claude' as PulseWorkspaceAgent,
    )
    setPulseAgent(storedAgent)

    const storedModel = safeGetItem(LS_PULSE_MODEL)
    if (storedModel && storedModel.length > 0) setPulseModel(storedModel)

    const storedPermission = validateStoredEnum(
      safeGetItem(LS_PULSE_PERMISSION),
      VALID_PERMISSIONS,
      'accept-edits' as PulseWorkspacePermission,
    )
    setPulsePermissionLevel(storedPermission)
  }, [])

  // ── localStorage: consolidated write effect ─────────────────────────────

  useEffect(() => {
    if (workspaceMode === null) {
      safeRemoveItem(LS_WORKSPACE_MODE)
    } else {
      safeSetItem(LS_WORKSPACE_MODE, workspaceMode)
    }
    safeSetItem(LS_PULSE_AGENT, pulseAgent)
    safeSetItem(LS_PULSE_MODEL, pulseModel ?? '')
    safeSetItem(LS_PULSE_PERMISSION, pulsePermissionLevel)
  }, [workspaceMode, pulseAgent, pulseModel, pulsePermissionLevel])

  // biome-ignore lint/correctness/useExhaustiveDependencies: pulseModel is read inside but intentionally excluded — re-probing on model change would create an infinite loop since the probe itself can set the model
  useEffect(() => {
    if (pathname?.startsWith('/reboot')) {
      setAcpConfigOptions([])
      return
    }

    let cancelled = false

    void probePulseConfigOptions({ agent: pulseAgent })
      .then((options) => {
        if (cancelled) return
        setAcpConfigOptions(options)

        if (options.length === 0) return
        const modelConfig = getAcpModelConfigOption(options)
        if (!modelConfig || modelConfig.options.length === 0) return
        const hasCurrent = modelConfig.options.some((option) => option.value === pulseModel)
        if (hasCurrent) return
        setPulseModel(modelConfig.currentValue || modelConfig.options[0]!.value)
      })
      .catch((error: unknown) => {
        if (cancelled) return
        console.warn('[pulse] config probe failed', error)
        setAcpConfigOptions([])
      })

    return () => {
      cancelled = true
    }
  }, [pathname, pulseAgent])

  const setCurrentJobIdTracked = useCallback((jobId: string | null) => {
    currentJobIdRef.current = jobId
    setCurrentJobId(jobId)
  }, [])

  useEffect(() => {
    const refs = {
      currentModeRef,
      currentInputRef,
      currentJobIdRef,
      selectedFileRef,
      crawlFilesRef,
      stdoutJsonRef,
      currentOutputDirRef,
      virtualFileContentByPathRef,
      runIdCounter,
    }
    const setters = {
      setLogLines,
      setMarkdownContent,
      setHasResults,
      setCrawlFiles: setCrawlFilesTracked,
      setCurrentOutputDir: setCurrentOutputDirTracked,
      setSelectedFile: setSelectedFileTracked,
      setCrawlProgress,
      setCommandMode,
      setStdoutLines,
      setStdoutJson: setStdoutJsonTracked,
      setVirtualFileContentByPath: setVirtualFileContentByPathTracked,
      setScreenshotFiles,
      setLifecycleEntries,
      setCancelResponse,
      setIsProcessing,
      setErrorMessage,
      setRecentRuns,
      setWorkspaceMode,
      setWorkspacePrompt,
      setWorkspacePromptVersion,
      setCurrentJobIdTracked,
    }
    return subscribe((msg: WsServerMsg) => handleWsMessage(msg, refs, setters))
  }, [
    setCrawlFilesTracked,
    setCurrentJobIdTracked,
    setCurrentOutputDirTracked,
    setSelectedFileTracked,
    setStdoutJsonTracked,
    setVirtualFileContentByPathTracked,
    subscribe,
  ])

  const selectFile = useCallback(
    (relativePath: string) => {
      setSelectedFileTracked(relativePath)
      setMarkdownContent('')
      const virtualContent = virtualFileContentByPathRef.current[relativePath]
      if (typeof virtualContent === 'string') {
        setMarkdownContent(virtualContent)
        return
      }
      send({ type: 'read_file', path: relativePath })
    },
    [send, setSelectedFileTracked],
  )

  const resetExecutionRuntime = useCallback(
    ({ hasResults, isProcessing }: { hasResults: boolean; isProcessing: boolean }) => {
      setMarkdownContent('')
      setLogLines([])
      setErrorMessage('')
      setHasResults(hasResults)
      setIsProcessing(isProcessing)
      setCrawlFilesTracked([])
      setSelectedFileTracked(null)
      setVirtualFileContentByPathTracked({})
      setCurrentOutputDirTracked(null)
      setCrawlProgress(null)
      setStdoutLines([])
      setStdoutJsonTracked([])
      setCommandMode(null)
      setScreenshotFiles([])
      setCurrentJobIdTracked(null)
      setLifecycleEntries([])
      setCancelResponse(null)
    },
    [
      setCrawlFilesTracked,
      setCurrentJobIdTracked,
      setCurrentOutputDirTracked,
      setSelectedFileTracked,
      setStdoutJsonTracked,
      setVirtualFileContentByPathTracked,
    ],
  )

  const resetWorkspaceRuntime = useCallback((mode: string | null) => {
    setWorkspaceMode(mode)
    setWorkspacePrompt(null)
    setWorkspacePromptVersion(0)
    setWorkspaceContext(null)
  }, [])

  const startExecution = useCallback(
    (mode: string, input?: string, options?: { preserveWorkspace?: boolean }) => {
      const preserveWorkspace = options?.preserveWorkspace === true
      currentModeRef.current = mode
      currentInputRef.current = input ?? ''
      setCurrentMode(mode)
      resetExecutionRuntime({ hasResults: true, isProcessing: true })
      if (!preserveWorkspace) {
        resetWorkspaceRuntime(null)
      }
    },
    [resetExecutionRuntime, resetWorkspaceRuntime],
  )

  const activateWorkspace = useCallback(
    (mode: string) => {
      currentModeRef.current = mode
      currentInputRef.current = ''
      setCurrentMode(mode)
      resetExecutionRuntime({ hasResults: false, isProcessing: false })
      resetWorkspaceRuntime(mode)
    },
    [resetExecutionRuntime, resetWorkspaceRuntime],
  )

  const submitWorkspacePrompt = useCallback((prompt: string) => {
    setWorkspaceMode('pulse')
    setHasResults(true)
    setWorkspaceResumeSessionId(null)
    setWorkspaceResumeVersion(0)
    setWorkspacePrompt(prompt)
    setWorkspacePromptVersion((prev) => prev + 1)
  }, [])

  const resumeWorkspaceSession = useCallback((sessionId: string) => {
    setWorkspaceMode('pulse')
    setHasResults(true)
    setWorkspacePrompt(null)
    setWorkspacePromptVersion(0)
    setWorkspaceResumeSessionId(sessionId)
    setWorkspaceResumeVersion((prev) => prev + 1)
  }, [])

  const clearWorkspaceResumeSession = useCallback(() => {
    setWorkspaceResumeSessionId(null)
    setWorkspaceResumeVersion(0)
  }, [])

  const deactivateWorkspace = useCallback(() => {
    currentModeRef.current = ''
    currentInputRef.current = ''
    setCurrentMode('')
    setWorkspaceMode(null)
    safeRemoveItem(LS_WORKSPACE_MODE)
    setWorkspacePrompt(null)
    setWorkspacePromptVersion(0)
    setWorkspaceResumeSessionId(null)
    setWorkspaceResumeVersion(0)
    setWorkspaceContext(null)
  }, [])

  const updateWorkspaceContext = useCallback((context: WorkspaceContextState | null) => {
    setWorkspaceContext(context)
  }, [])

  const executionState = useMemo<WsMessagesExecutionState>(
    () => ({
      markdownContent,
      logLines,
      errorMessage,
      recentRuns,
      isProcessing,
      hasResults,
      currentMode,
      crawlFiles,
      selectedFile,
      crawlProgress,
      stdoutLines,
      stdoutJson,
      commandMode,
      screenshotFiles,
      currentJobId,
      lifecycleEntries,
      cancelResponse,
    }),
    [
      markdownContent,
      logLines,
      errorMessage,
      recentRuns,
      isProcessing,
      hasResults,
      currentMode,
      crawlFiles,
      selectedFile,
      crawlProgress,
      stdoutLines,
      stdoutJson,
      commandMode,
      screenshotFiles,
      currentJobId,
      lifecycleEntries,
      cancelResponse,
    ],
  )

  const workspaceState = useMemo<WsMessagesWorkspaceState>(
    () => ({
      workspaceMode,
      workspacePrompt,
      workspacePromptVersion,
      workspaceResumeSessionId,
      workspaceResumeVersion,
      workspaceContext,
      pulseAgent,
      pulseModel,
      pulsePermissionLevel,
      acpConfigOptions,
    }),
    [
      workspaceMode,
      workspacePrompt,
      workspacePromptVersion,
      workspaceResumeSessionId,
      workspaceResumeVersion,
      workspaceContext,
      pulseAgent,
      pulseModel,
      pulsePermissionLevel,
      acpConfigOptions,
    ],
  )

  const actions = useMemo<WsMessagesActions>(
    () => ({
      selectFile,
      setPulseAgent,
      setPulseModel,
      setPulsePermissionLevel,
      setAcpConfigOptions,
      activateWorkspace,
      submitWorkspacePrompt,
      resumeWorkspaceSession,
      clearWorkspaceResumeSession,
      deactivateWorkspace,
      updateWorkspaceContext,
      startExecution,
    }),
    [
      selectFile,
      activateWorkspace,
      submitWorkspacePrompt,
      resumeWorkspaceSession,
      clearWorkspaceResumeSession,
      deactivateWorkspace,
      updateWorkspaceContext,
      startExecution,
    ],
  )

  const value = useMemo<WsMessagesContextValue>(
    () => ({
      ...executionState,
      ...workspaceState,
      ...actions,
    }),
    [executionState, workspaceState, actions],
  )

  return { executionState, workspaceState, actions, value }
}
