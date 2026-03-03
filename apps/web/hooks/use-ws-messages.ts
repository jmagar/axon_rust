'use client'

import type React from 'react'
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import type { CrawlFile, WsLifecycleEntry, WsServerMsg } from '@/lib/ws-protocol'
import { handleWsMessage } from './ws-messages/handlers'
import { makeInitialRuntimeState, reduceRuntimeState } from './ws-messages/runtime'
import type {
  CancelResponseState,
  CrawlProgress,
  LogLine,
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
  PulseWorkspaceModel,
  PulseWorkspacePermission,
  RecentRun,
  ScreenshotFile,
  WorkspaceContextState,
}

export function useWsMessagesProvider() {
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
  const [workspaceContext, setWorkspaceContext] = useState<WorkspaceContextState | null>(null)
  const [pulseModel, setPulseModel] = useState<PulseWorkspaceModel>('sonnet')
  const [pulsePermissionLevel, setPulsePermissionLevel] =
    useState<PulseWorkspacePermission>('accept-edits')

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

  useEffect(() => {
    try {
      if (workspaceMode === null) {
        window.localStorage.removeItem('axon.web.workspace-mode')
      } else {
        window.localStorage.setItem('axon.web.workspace-mode', workspaceMode)
      }
    } catch {
      // Ignore storage errors.
    }
  }, [workspaceMode])

  useEffect(() => {
    try {
      const stored = window.localStorage.getItem('axon.web.workspace-mode')
      if (stored) setWorkspaceMode(stored)
    } catch {
      /* ignore */
    }
  }, [])

  useEffect(() => {
    try {
      const m = localStorage.getItem('axon.web.pulse-model') as PulseWorkspaceModel
      if (m && ['sonnet', 'opus', 'haiku'].includes(m)) setPulseModel(m)
      const p = localStorage.getItem('axon.web.pulse-permission') as PulseWorkspacePermission
      if (p && ['plan', 'accept-edits', 'bypass-permissions'].includes(p)) {
        setPulsePermissionLevel(p)
      }
    } catch {
      /* ignore */
    }
  }, [])

  useEffect(() => {
    try {
      localStorage.setItem('axon.web.pulse-model', pulseModel)
    } catch {
      /* ignore */
    }
  }, [pulseModel])

  useEffect(() => {
    try {
      localStorage.setItem('axon.web.pulse-permission', pulsePermissionLevel)
    } catch {
      /* ignore */
    }
  }, [pulsePermissionLevel])

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
    setWorkspacePrompt(prompt)
    setWorkspacePromptVersion((prev) => prev + 1)
  }, [])

  const deactivateWorkspace = useCallback(() => {
    currentModeRef.current = ''
    currentInputRef.current = ''
    setCurrentMode('')
    setWorkspaceMode(null)
    try {
      window.localStorage.removeItem('axon.web.workspace-mode')
    } catch {
      // Ignore storage errors.
    }
    setWorkspacePrompt(null)
    setWorkspacePromptVersion(0)
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
      workspaceContext,
      pulseModel,
      pulsePermissionLevel,
    }),
    [
      workspaceMode,
      workspacePrompt,
      workspacePromptVersion,
      workspaceContext,
      pulseModel,
      pulsePermissionLevel,
    ],
  )

  const actions = useMemo<WsMessagesActions>(
    () => ({
      selectFile,
      setPulseModel,
      setPulsePermissionLevel,
      activateWorkspace,
      submitWorkspacePrompt,
      deactivateWorkspace,
      updateWorkspaceContext,
      startExecution,
    }),
    [
      selectFile,
      activateWorkspace,
      submitWorkspacePrompt,
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
