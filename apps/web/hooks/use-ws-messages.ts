'use client'

import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import type { CrawlFile, WsServerMsg } from '@/lib/ws-protocol'

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
  /** Accumulated raw text lines from stdout */
  stdoutLines: string[]
  /** Accumulated parsed JSON objects from stdout */
  stdoutJson: unknown[]
  /** Command mode reported by command_start message */
  commandMode: string | null
  /** Screenshot files from screenshot command */
  screenshotFiles: ScreenshotFile[]
  /** Job ID for the current/last crawl (used for download routes) */
  currentJobId: string | null
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
          if (msg.job_id) {
            setCurrentJobId(msg.job_id)
          }
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
            setCurrentJobId(msg.job_id)
          }
          break
        case 'command_start':
          setCommandMode(msg.mode)
          setStdoutLines([])
          setStdoutJson([])
          break
        case 'stdout_json': {
          setStdoutJson((prev) => {
            const next = [...prev, msg.data]
            return next.length > MAX_STDOUT_ITEMS ? next.slice(-MAX_STDOUT_ITEMS) : next
          })
          setHasResults(true)
          break
        }
        case 'stdout_line': {
          setStdoutLines((prev) => {
            const next = [...prev, msg.line]
            return next.length > MAX_STDOUT_ITEMS ? next.slice(-MAX_STDOUT_ITEMS) : next
          })
          setHasResults(true)
          break
        }
        case 'screenshot_files':
          setScreenshotFiles(msg.files)
          setHasResults(true)
          break
        case 'output': {
          // Legacy server message shape: treat as stdout text so content renderers
          // still work even if the backend emits `output` instead of `stdout_line`.
          const line = (msg.line ?? '').toString()
          if (!line.trim()) break
          setStdoutLines((prev) => {
            const next = [...prev, line]
            return next.length > MAX_STDOUT_ITEMS ? next.slice(-MAX_STDOUT_ITEMS) : next
          })
          setHasResults(true)
          break
        }
        case 'stats':
          // Handled by DockerStats component
          break
        case 'done': {
          setIsProcessing(false)
          setRecentRuns((prev) => {
            const run: RecentRun = {
              id: `run-${++runIdCounter.current}`,
              status: msg.exit_code === 0 ? 'done' : 'failed',
              mode: currentModeRef.current,
              target: currentInputRef.current,
              duration: `${(msg.elapsed_ms / 1000).toFixed(1)}s`,
              lines: 0,
              time: new Date().toLocaleTimeString(),
            }
            return [run, ...prev].slice(0, 20)
          })
          break
        }
        case 'error': {
          setIsProcessing(false)
          setErrorMessage(msg.message)
          setRecentRuns((prev) => {
            const run: RecentRun = {
              id: `run-${++runIdCounter.current}`,
              status: 'failed',
              mode: currentModeRef.current,
              target: currentInputRef.current,
              duration: msg.elapsed_ms ? `${(msg.elapsed_ms / 1000).toFixed(1)}s` : '0s',
              lines: 0,
              time: new Date().toLocaleTimeString(),
            }
            return [run, ...prev].slice(0, 20)
          })
          break
        }
      }
    })
  }, [subscribe])

  const selectFile = useCallback(
    (relativePath: string) => {
      setSelectedFile(relativePath)
      setMarkdownContent('')
      send({ type: 'read_file', path: relativePath })
    },
    [send],
  )

  const startExecution = useCallback((mode: string, input?: string) => {
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
    setCurrentJobId(null)
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
    startExecution,
  }
}
