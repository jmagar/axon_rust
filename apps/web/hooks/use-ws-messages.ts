'use client'

import {
  createContext,
  type Dispatch,
  type SetStateAction,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import {
  type CrawlFile,
  lifecycleFromJobProgress,
  lifecycleFromJobStatus,
  type WsLifecycleEntry,
  type WsServerMsg,
} from '@/lib/ws-protocol'

export interface LogLine {
  content: string
  timestamp: number
}

export interface RecentRun {
  id: string
  status: 'done' | 'failed'
  mode: string
  target: string
  duration: string
  lines: number
  time: string
}

export interface CrawlProgress {
  pages_crawled: number
  pages_discovered: number
  md_created: number
  thin_md: number
  phase: string
}

export interface ScreenshotFile {
  path: string
  name: string
  serve_url?: string
  size_bytes?: number
  url?: string
}

export interface CancelResponseState {
  ok: boolean
  message: string
  mode?: string
  job_id?: string
}

interface WsMessagesContextValue {
  /** Markdown content from the output file (set by file_content message) */
  markdownContent: string
  /** Log lines from stderr (progress, options, spinners) */
  logLines: LogLine[]
  /** Error message if the command failed */
  errorMessage: string
  recentRuns: RecentRun[]
  isProcessing: boolean
  hasResults: boolean
  /** Current command mode (e.g., 'scrape', 'crawl') */
  currentMode: string
  /** Crawl file list from manifest */
  crawlFiles: CrawlFile[]
  /** Currently selected file relative_path */
  selectedFile: string | null
  /** Request a file from the crawl output */
  selectFile: (relativePath: string) => void
  /** Live crawl progress from job polling */
  crawlProgress: CrawlProgress | null
  /** Accumulated raw text lines from v2 command output */
  stdoutLines: string[]
  /** Accumulated parsed JSON objects from v2 command output */
  stdoutJson: unknown[]
  /** Command mode reported by command.start message */
  commandMode: string | null
  /** Screenshot files from screenshot command */
  screenshotFiles: ScreenshotFile[]
  /** Job ID for the current/last crawl (used for download routes) */
  currentJobId: string | null
  /** Unified v2/legacy lifecycle entries for job-based renderers. */
  lifecycleEntries: WsLifecycleEntry[]
  /** Latest cancel response status from v2 job.cancel.response. */
  cancelResponse: CancelResponseState | null
  /** Active workspace mode (currently pulse) when omnibox enters workspace flow. */
  workspaceMode: string | null
  /** Last submitted workspace prompt payload from omnibox. */
  workspacePrompt: string | null
  /** Monotonic counter to trigger workspace prompt effects even for identical prompts. */
  workspacePromptVersion: number
  activateWorkspace: (mode: string) => void
  submitWorkspacePrompt: (prompt: string) => void
  deactivateWorkspace: () => void
  startExecution: (mode: string, input?: string) => void
}

const WsMessagesContext = createContext<WsMessagesContextValue | null>(null)

export function useWsMessages() {
  const ctx = useContext(WsMessagesContext)
  if (!ctx) throw new Error('useWsMessages must be used within WsMessagesProvider')
  return ctx
}

export { WsMessagesContext }

/** Cap stdout accumulators to prevent unbounded memory growth.
 * Status payloads can be very large JSON documents, so keep a larger window. */
const MAX_STDOUT_ITEMS = 50000
const WORKSPACE_PROMPT_DEBOUNCE_MS = 250

function pushCapped<T>(items: T[], item: T): T[] {
  const next = [...items, item]
  return next.length > MAX_STDOUT_ITEMS ? next.slice(-MAX_STDOUT_ITEMS) : next
}

export interface WsMessagesRuntimeState {
  currentJobId: string | null
  commandMode: string | null
  markdownContent: string
  crawlProgress: CrawlProgress | null
  screenshotFiles: ScreenshotFile[]
  lifecycleEntries: WsLifecycleEntry[]
  stdoutJson: unknown[]
  cancelResponse: CancelResponseState | null
}

export function makeInitialRuntimeState(): WsMessagesRuntimeState {
  return {
    currentJobId: null,
    commandMode: null,
    markdownContent: '',
    crawlProgress: null,
    screenshotFiles: [],
    lifecycleEntries: [],
    stdoutJson: [],
    cancelResponse: null,
  }
}

export function reduceRuntimeState(
  state: WsMessagesRuntimeState,
  msg: WsServerMsg,
): WsMessagesRuntimeState {
  const next = { ...state }
  switch (msg.type) {
    case 'command.output.json': {
      const maybeJobData =
        msg.data.data && typeof msg.data.data === 'object' && !Array.isArray(msg.data.data)
          ? (msg.data.data as Record<string, unknown>)
          : null
      const maybeJobId =
        maybeJobData && typeof maybeJobData.job_id === 'string' ? maybeJobData.job_id : null
      if (maybeJobId) next.currentJobId = maybeJobId
      next.stdoutJson = pushCapped(state.stdoutJson, msg.data.data)
      return next
    }
    case 'command.start':
      next.commandMode = msg.data.ctx.mode
      next.stdoutJson = []
      return next
    case 'command.output.line':
      return next
    case 'job.status': {
      const lifecycle = lifecycleFromJobStatus(msg, state.currentJobId)
      if (!lifecycle) return next
      next.currentJobId = lifecycle.job_id
      next.lifecycleEntries = pushCapped(state.lifecycleEntries, lifecycle)
      next.stdoutJson = pushCapped(state.stdoutJson, lifecycle)
      return next
    }
    case 'job.progress': {
      const lifecycle = lifecycleFromJobProgress(msg, state.currentJobId)
      if (!lifecycle) return next
      next.lifecycleEntries = pushCapped(state.lifecycleEntries, lifecycle)
      next.stdoutJson = pushCapped(state.stdoutJson, lifecycle)
      return next
    }
    case 'artifact.list':
      next.screenshotFiles = msg.data.artifacts
        .filter((artifact) => typeof artifact.path === 'string' && artifact.path.length > 0)
        .map((artifact) => {
          const path = artifact.path as string
          const pathParts = path.split('/')
          const name = pathParts[pathParts.length - 1] || path
          return {
            path,
            name,
            serve_url: artifact.download_url,
            size_bytes: artifact.size_bytes,
          }
        })
      return next
    case 'artifact.content':
      next.markdownContent = msg.data.content
      return next
    case 'job.cancel.response':
      next.cancelResponse = {
        ok: msg.data.payload.ok,
        message:
          msg.data.payload.message ??
          (msg.data.payload.ok ? 'Cancel request accepted' : 'Cancel request failed'),
        mode: msg.data.payload.mode,
        job_id: msg.data.payload.job_id,
      }
      return next
    case 'crawl_progress':
      next.crawlProgress = {
        pages_crawled: msg.pages_crawled,
        pages_discovered: msg.pages_discovered,
        md_created: msg.md_created,
        thin_md: msg.thin_md,
        phase: msg.phase,
      }
      return next
    default:
      return next
  }
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
  const [workspaceMode, setWorkspaceMode] = useState<string | null>(null)
  const [workspacePrompt, setWorkspacePrompt] = useState<string | null>(null)
  const [workspacePromptVersion, setWorkspacePromptVersion] = useState(0)
  const workspacePromptDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const setCurrentJobIdTracked = useCallback((jobId: string | null) => {
    currentJobIdRef.current = jobId
    setCurrentJobId(jobId)
  }, [])

  useEffect(() => {
    return subscribe((msg: WsServerMsg) => {
      switch (msg.type) {
        case 'log':
          setLogLines((prev) => [...prev, { content: msg.line, timestamp: Date.now() }])
          break
        case 'file_content':
          setMarkdownContent(msg.content)
          setHasResults(true)
          break
        case 'crawl_files':
          setCrawlFiles(msg.files)
          setHasResults(true)
          setCurrentJobIdTracked(msg.job_id ?? null)
          // First file is auto-loaded by the backend
          if (msg.files.length > 0) {
            setSelectedFile(msg.files[0].relative_path)
          }
          break
        case 'crawl_progress':
          setCrawlProgress({
            pages_crawled: msg.pages_crawled,
            pages_discovered: msg.pages_discovered,
            md_created: msg.md_created,
            thin_md: msg.thin_md,
            phase: msg.phase,
          })
          if (msg.job_id) {
            setCurrentJobIdTracked(msg.job_id)
          }
          break
        case 'command.start':
          setCommandMode(msg.data.ctx.mode)
          setStdoutLines([])
          setStdoutJson([])
          break
        case 'command.output.json': {
          const maybeJobData =
            msg.data.data && typeof msg.data.data === 'object' && !Array.isArray(msg.data.data)
              ? (msg.data.data as Record<string, unknown>)
              : null
          const maybeJobId =
            maybeJobData && typeof maybeJobData.job_id === 'string' ? maybeJobData.job_id : null
          if (maybeJobId) {
            setCurrentJobIdTracked(maybeJobId)
          }
          setStdoutJson((prev) => {
            const next = [...prev, msg.data.data]
            return next.length > MAX_STDOUT_ITEMS ? next.slice(-MAX_STDOUT_ITEMS) : next
          })
          setHasResults(true)
          break
        }
        case 'command.output.line': {
          setStdoutLines((prev) => {
            const next = [...prev, msg.data.line]
            return next.length > MAX_STDOUT_ITEMS ? next.slice(-MAX_STDOUT_ITEMS) : next
          })
          setHasResults(true)
          break
        }
        case 'job.status': {
          const lifecycle = lifecycleFromJobStatus(msg, currentJobIdRef.current)
          if (lifecycle) {
            setCurrentJobIdTracked(lifecycle.job_id)
            setLifecycleEntries((prev) => {
              const next = [...prev, lifecycle]
              return next.length > MAX_STDOUT_ITEMS ? next.slice(-MAX_STDOUT_ITEMS) : next
            })
            setStdoutJson((prev) => {
              const next = [...prev, lifecycle]
              return next.length > MAX_STDOUT_ITEMS ? next.slice(-MAX_STDOUT_ITEMS) : next
            })
          }
          setHasResults(true)
          break
        }
        case 'job.progress': {
          const lifecycle = lifecycleFromJobProgress(msg, currentJobIdRef.current)
          if (lifecycle) {
            setLifecycleEntries((prev) => {
              const next = [...prev, lifecycle]
              return next.length > MAX_STDOUT_ITEMS ? next.slice(-MAX_STDOUT_ITEMS) : next
            })
            setStdoutJson((prev) => {
              const next = [...prev, lifecycle]
              return next.length > MAX_STDOUT_ITEMS ? next.slice(-MAX_STDOUT_ITEMS) : next
            })
          }
          setHasResults(true)
          break
        }
        case 'artifact.list':
          setScreenshotFiles(
            msg.data.artifacts
              .filter((artifact) => typeof artifact.path === 'string' && artifact.path.length > 0)
              .map((artifact) => {
                const path = artifact.path as string
                const pathParts = path.split('/')
                const name = pathParts[pathParts.length - 1] || path
                return {
                  path,
                  name,
                  serve_url: artifact.download_url,
                  size_bytes: artifact.size_bytes,
                }
              }),
          )
          setHasResults(true)
          break
        case 'artifact.content':
          setMarkdownContent(msg.data.content)
          setHasResults(true)
          break
        case 'job.cancel.response':
          setCancelResponse({
            ok: msg.data.payload.ok,
            message:
              msg.data.payload.message ??
              (msg.data.payload.ok ? 'Cancel request accepted' : 'Cancel request failed'),
            mode: msg.data.payload.mode,
            job_id: msg.data.payload.job_id,
          })
          setStatusResultLine(setLogLines, msg.data.payload.ok, msg.data.payload.message)
          break
        case 'stats':
          // Handled by DockerStats component
          break
        case 'command.done': {
          setIsProcessing(false)
          setRecentRuns((prev) => {
            const run: RecentRun = {
              id: `run-${++runIdCounter.current}`,
              status: msg.data.payload.exit_code === 0 ? 'done' : 'failed',
              mode: currentModeRef.current,
              target: currentInputRef.current,
              duration: `${((msg.data.payload.elapsed_ms ?? 0) / 1000).toFixed(1)}s`,
              lines: 0,
              time: new Date().toLocaleTimeString(),
            }
            return [run, ...prev].slice(0, 20)
          })
          break
        }
        case 'command.error': {
          setIsProcessing(false)
          setErrorMessage(msg.data.payload.message)
          setRecentRuns((prev) => {
            const run: RecentRun = {
              id: `run-${++runIdCounter.current}`,
              status: 'failed',
              mode: currentModeRef.current,
              target: currentInputRef.current,
              duration: msg.data.payload.elapsed_ms
                ? `${(msg.data.payload.elapsed_ms / 1000).toFixed(1)}s`
                : '0s',
              lines: 0,
              time: new Date().toLocaleTimeString(),
            }
            return [run, ...prev].slice(0, 20)
          })
          break
        }
      }
    })
  }, [setCurrentJobIdTracked, subscribe])

  const selectFile = useCallback(
    (relativePath: string) => {
      setSelectedFile(relativePath)
      setMarkdownContent('')
      send({ type: 'read_file', path: relativePath })
    },
    [send],
  )

  const startExecution = useCallback(
    (mode: string, input?: string) => {
      currentModeRef.current = mode
      currentInputRef.current = input ?? ''
      setCurrentMode(mode)
      setMarkdownContent('')
      setLogLines([])
      setErrorMessage('')
      setIsProcessing(true)
      setHasResults(true)
      setCrawlFiles([])
      setSelectedFile(null)
      setCrawlProgress(null)
      setStdoutLines([])
      setStdoutJson([])
      setCommandMode(null)
      setScreenshotFiles([])
      setCurrentJobIdTracked(null)
      setLifecycleEntries([])
      setCancelResponse(null)
      setWorkspaceMode(null)
      setWorkspacePrompt(null)
      setWorkspacePromptVersion(0)
    },
    [setCurrentJobIdTracked],
  )

  const activateWorkspace = useCallback(
    (mode: string) => {
      currentModeRef.current = mode
      currentInputRef.current = ''
      setCurrentMode(mode)
      setMarkdownContent('')
      setLogLines([])
      setErrorMessage('')
      setHasResults(true)
      setIsProcessing(false)
      setCrawlFiles([])
      setSelectedFile(null)
      setCrawlProgress(null)
      setStdoutLines([])
      setStdoutJson([])
      setCommandMode(null)
      setScreenshotFiles([])
      setCurrentJobIdTracked(null)
      setLifecycleEntries([])
      setCancelResponse(null)
      setWorkspaceMode(mode)
      setWorkspacePrompt(null)
      setWorkspacePromptVersion(0)
    },
    [setCurrentJobIdTracked],
  )

  const submitWorkspacePrompt = useCallback((prompt: string) => {
    setHasResults(true)
    if (workspacePromptDebounceRef.current) {
      clearTimeout(workspacePromptDebounceRef.current)
    }
    workspacePromptDebounceRef.current = setTimeout(() => {
      setWorkspacePrompt(prompt)
      setWorkspacePromptVersion((prev) => prev + 1)
      workspacePromptDebounceRef.current = null
    }, WORKSPACE_PROMPT_DEBOUNCE_MS)
  }, [])

  const deactivateWorkspace = useCallback(() => {
    currentModeRef.current = ''
    currentInputRef.current = ''
    setCurrentMode('')
    setWorkspaceMode(null)
    if (workspacePromptDebounceRef.current) {
      clearTimeout(workspacePromptDebounceRef.current)
      workspacePromptDebounceRef.current = null
    }
    setWorkspacePrompt(null)
    setWorkspacePromptVersion(0)
  }, [])

  useEffect(() => {
    return () => {
      if (workspacePromptDebounceRef.current) {
        clearTimeout(workspacePromptDebounceRef.current)
        workspacePromptDebounceRef.current = null
      }
    }
  }, [])

  return {
    markdownContent,
    logLines,
    errorMessage,
    recentRuns,
    isProcessing,
    hasResults,
    currentMode,
    crawlFiles,
    selectedFile,
    selectFile,
    crawlProgress,
    stdoutLines,
    stdoutJson,
    commandMode,
    screenshotFiles,
    currentJobId,
    lifecycleEntries,
    cancelResponse,
    workspaceMode,
    workspacePrompt,
    workspacePromptVersion,
    activateWorkspace,
    submitWorkspacePrompt,
    deactivateWorkspace,
    startExecution,
  }
}

function setStatusResultLine(
  setLogLines: Dispatch<SetStateAction<LogLine[]>>,
  ok: boolean,
  message?: string,
) {
  const line = message ?? (ok ? 'Cancel request accepted' : 'Cancel request failed')
  setLogLines((prev) => [...prev, { content: `[cancel] ${line}`, timestamp: Date.now() }])
}
