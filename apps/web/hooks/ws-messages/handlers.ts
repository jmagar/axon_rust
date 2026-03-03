'use client'

import type { Dispatch, SetStateAction } from 'react'
import type { CrawlFile, WsLifecycleEntry, WsServerMsg } from '@/lib/ws-protocol'
import { lifecycleFromJobProgress, lifecycleFromJobStatus } from '@/lib/ws-protocol'
import {
  buildWorkspaceHandoffPrompt,
  MAX_LOG_LINES,
  pushCapped,
  setStatusResultLine,
  summarizeJsonValue,
  toCancelResponse,
  toCrawlProgress,
  toScreenshotFiles,
} from './runtime'
import type {
  CancelResponseState,
  CrawlProgress,
  LogLine,
  RecentRun,
  ScreenshotFile,
} from './types'

export interface MessageHandlerRefs {
  currentModeRef: React.RefObject<string>
  currentInputRef: React.RefObject<string>
  currentJobIdRef: React.RefObject<string | null>
  selectedFileRef: React.RefObject<string | null>
  crawlFilesRef: React.RefObject<CrawlFile[]>
  stdoutJsonRef: React.RefObject<unknown[]>
  currentOutputDirRef: React.RefObject<string | null>
  virtualFileContentByPathRef: React.RefObject<Record<string, string>>
  runIdCounter: React.RefObject<number>
}

export interface MessageHandlerSetters {
  setLogLines: Dispatch<SetStateAction<LogLine[]>>
  setMarkdownContent: Dispatch<SetStateAction<string>>
  setHasResults: Dispatch<SetStateAction<boolean>>
  setCrawlFiles: Dispatch<SetStateAction<CrawlFile[]>>
  setCurrentOutputDir: Dispatch<SetStateAction<string | null>>
  setSelectedFile: Dispatch<SetStateAction<string | null>>
  setCrawlProgress: Dispatch<SetStateAction<CrawlProgress | null>>
  setCommandMode: Dispatch<SetStateAction<string | null>>
  setStdoutLines: Dispatch<SetStateAction<string[]>>
  setStdoutJson: Dispatch<SetStateAction<unknown[]>>
  setVirtualFileContentByPath: Dispatch<SetStateAction<Record<string, string>>>
  setScreenshotFiles: Dispatch<SetStateAction<ScreenshotFile[]>>
  setLifecycleEntries: Dispatch<SetStateAction<WsLifecycleEntry[]>>
  setCancelResponse: Dispatch<SetStateAction<CancelResponseState | null>>
  setIsProcessing: Dispatch<SetStateAction<boolean>>
  setErrorMessage: Dispatch<SetStateAction<string>>
  setRecentRuns: Dispatch<SetStateAction<RecentRun[]>>
  setWorkspaceMode: Dispatch<SetStateAction<string | null>>
  setWorkspacePrompt: Dispatch<SetStateAction<string | null>>
  setWorkspacePromptVersion: Dispatch<SetStateAction<number>>
  setCurrentJobIdTracked: (jobId: string | null) => void
}

// ── Helpers ─────────────────────────────────────────────────────────────────

function buildRecentRun(
  runIdCounter: React.RefObject<number>,
  status: 'done' | 'failed',
  mode: string,
  target: string,
  elapsedMs?: number,
): RecentRun {
  return {
    id: `run-${++runIdCounter.current}`,
    status,
    mode,
    target,
    duration: `${((elapsedMs ?? 0) / 1000).toFixed(1)}s`,
    lines: 0,
    time: new Date().toLocaleTimeString(),
  }
}

function prependRecentRun(setRecentRuns: Dispatch<SetStateAction<RecentRun[]>>, run: RecentRun) {
  setRecentRuns((prev) => [run, ...prev].slice(0, 20))
}

// ── Per-type handlers ───────────────────────────────────────────────────────

function handleCommandOutputJson(
  msg: Extract<WsServerMsg, { type: 'command.output.json' }>,
  refs: MessageHandlerRefs,
  setters: MessageHandlerSetters,
) {
  const { currentInputRef, virtualFileContentByPathRef } = refs
  const {
    setCrawlFiles,
    setCurrentOutputDir,
    setHasResults,
    setStdoutJson,
    setVirtualFileContentByPath,
    setCurrentJobIdTracked,
  } = setters

  const maybeJobData =
    msg.data.data && typeof msg.data.data === 'object' && !Array.isArray(msg.data.data)
      ? (msg.data.data as Record<string, unknown>)
      : null
  const maybeJobId =
    maybeJobData && typeof maybeJobData.job_id === 'string' ? maybeJobData.job_id : null
  if (maybeJobId) {
    setCurrentJobIdTracked(maybeJobId)
  }
  if (maybeJobData && typeof maybeJobData.output_dir === 'string') {
    setCurrentOutputDir(maybeJobData.output_dir)
  }
  if (msg.data.ctx.mode === 'scrape' && maybeJobData) {
    const markdown = typeof maybeJobData.markdown === 'string' ? maybeJobData.markdown : null
    const url = typeof maybeJobData.url === 'string' ? maybeJobData.url : currentInputRef.current
    if (markdown && markdown.length > 0) {
      const basename = url.replace(/^https?:\/\//i, '').replace(/[^a-z0-9]+/gi, '-')
      const relativePath = `virtual/scrape-${basename || 'result'}.md`
      virtualFileContentByPathRef.current = {
        ...virtualFileContentByPathRef.current,
        [relativePath]: markdown,
      }
      setVirtualFileContentByPath((prev) => ({
        ...prev,
        [relativePath]: markdown,
      }))
      setCrawlFiles((prev) => {
        const withoutExisting = prev.filter((f) => f.relative_path !== relativePath)
        return [
          ...withoutExisting,
          { url, relative_path: relativePath, markdown_chars: markdown.length },
        ]
      })
    }
  }
  if (msg.data.ctx.mode === 'extract' && maybeJobData) {
    const relativePath = 'virtual/extract-result.json'
    const serialized = summarizeJsonValue(maybeJobData)
    setVirtualFileContentByPath((prev) => ({
      ...prev,
      [relativePath]: serialized,
    }))
    setCrawlFiles((prev) => {
      const withoutExisting = prev.filter((f) => f.relative_path !== relativePath)
      return [
        ...withoutExisting,
        {
          url: currentInputRef.current || 'extract://result',
          relative_path: relativePath,
          markdown_chars: serialized.length,
        },
      ]
    })
  }
  setStdoutJson((prev) => pushCapped(prev, msg.data.data))
  setHasResults(true)
}

function handleCommandDone(
  msg: Extract<WsServerMsg, { type: 'command.done' }>,
  refs: MessageHandlerRefs,
  setters: MessageHandlerSetters,
) {
  const {
    currentModeRef,
    currentInputRef,
    crawlFilesRef,
    stdoutJsonRef,
    currentOutputDirRef,
    virtualFileContentByPathRef,
    runIdCounter,
  } = refs
  const {
    setIsProcessing,
    setHasResults,
    setRecentRuns,
    setWorkspaceMode,
    setWorkspacePrompt,
    setWorkspacePromptVersion,
  } = setters

  setIsProcessing(false)
  if (
    msg.data.payload.exit_code === 0 &&
    (currentModeRef.current === 'scrape' ||
      currentModeRef.current === 'crawl' ||
      currentModeRef.current === 'extract')
  ) {
    const handoffPrompt = buildWorkspaceHandoffPrompt({
      modeLabel: currentModeRef.current,
      filesSnapshot: crawlFilesRef.current,
      targetInput: currentInputRef.current.trim(),
      outputDir: currentOutputDirRef.current,
      stdoutSnapshot: stdoutJsonRef.current,
      virtualFileContentByPath: virtualFileContentByPathRef.current,
    })
    setWorkspaceMode('pulse')
    setHasResults(true)
    setWorkspacePrompt(handoffPrompt)
    setWorkspacePromptVersion((prev) => prev + 1)
  }
  const run = buildRecentRun(
    runIdCounter,
    msg.data.payload.exit_code === 0 ? 'done' : 'failed',
    currentModeRef.current,
    currentInputRef.current,
    msg.data.payload.elapsed_ms,
  )
  prependRecentRun(setRecentRuns, run)
}

function handleCommandError(
  msg: Extract<WsServerMsg, { type: 'command.error' }>,
  refs: MessageHandlerRefs,
  setters: MessageHandlerSetters,
) {
  const { currentModeRef, currentInputRef, runIdCounter } = refs
  const { setIsProcessing, setErrorMessage, setRecentRuns } = setters

  setIsProcessing(false)
  setErrorMessage(msg.data.payload.message)
  const run = buildRecentRun(
    runIdCounter,
    'failed',
    currentModeRef.current,
    currentInputRef.current,
    msg.data.payload.elapsed_ms,
  )
  prependRecentRun(setRecentRuns, run)
}

// ── Main dispatcher ─────────────────────────────────────────────────────────

export function handleWsMessage(
  msg: WsServerMsg,
  refs: MessageHandlerRefs,
  setters: MessageHandlerSetters,
): void {
  if (msg.type === 'stats') return

  switch (msg.type) {
    case 'log':
      setters.setLogLines((prev) =>
        pushCapped(prev, { content: msg.line, timestamp: Date.now() }, MAX_LOG_LINES),
      )
      break
    case 'file_content':
      setters.setMarkdownContent(msg.content)
      setters.setHasResults(true)
      break
    case 'crawl_files':
      setters.setCrawlFiles(msg.files)
      setters.setCurrentOutputDir(msg.output_dir)
      setters.setHasResults(true)
      setters.setCurrentJobIdTracked(msg.job_id ?? null)
      setters.setSelectedFile((prev) =>
        prev && msg.files.some((file) => file.relative_path === prev) ? prev : null,
      )
      break
    case 'crawl_progress':
      setters.setCrawlProgress(toCrawlProgress(msg))
      if (msg.job_id) setters.setCurrentJobIdTracked(msg.job_id)
      break
    case 'command.start':
      setters.setCommandMode(msg.data.ctx.mode)
      setters.setStdoutLines([])
      setters.setStdoutJson([])
      break
    case 'command.output.json':
      handleCommandOutputJson(msg, refs, setters)
      break
    case 'command.output.line':
      setters.setStdoutLines((prev) => pushCapped(prev, msg.data.line))
      setters.setHasResults(true)
      break
    case 'job.status': {
      const lifecycle = lifecycleFromJobStatus(msg, refs.currentJobIdRef.current)
      if (lifecycle) {
        setters.setCurrentJobIdTracked(lifecycle.job_id)
        setters.setLifecycleEntries((prev) => pushCapped(prev, lifecycle))
        setters.setStdoutJson((prev) => pushCapped(prev, lifecycle))
      }
      setters.setHasResults(true)
      break
    }
    case 'job.progress': {
      const lifecycle = lifecycleFromJobProgress(msg, refs.currentJobIdRef.current)
      if (lifecycle) {
        setters.setLifecycleEntries((prev) => pushCapped(prev, lifecycle))
        setters.setStdoutJson((prev) => pushCapped(prev, lifecycle))
      }
      setters.setHasResults(true)
      break
    }
    case 'artifact.list':
      setters.setScreenshotFiles(toScreenshotFiles(msg.data.artifacts))
      setters.setHasResults(true)
      break
    case 'artifact.content':
      if (
        (refs.currentModeRef.current === 'scrape' ||
          refs.currentModeRef.current === 'crawl' ||
          refs.currentModeRef.current === 'extract') &&
        !refs.selectedFileRef.current
      ) {
        break
      }
      setters.setMarkdownContent(msg.data.content)
      setters.setHasResults(true)
      break
    case 'job.cancel.response':
      setters.setCancelResponse(toCancelResponse(msg.data.payload))
      setStatusResultLine(setters.setLogLines, msg.data.payload.ok, msg.data.payload.message)
      break
    case 'command.done':
      handleCommandDone(msg, refs, setters)
      break
    case 'command.error':
      handleCommandError(msg, refs, setters)
      break
  }
}
