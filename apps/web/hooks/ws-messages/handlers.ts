'use client'

import type { Dispatch, SetStateAction } from 'react'
import type { CrawlFile, WsLifecycleEntry, WsServerMsg } from '@/lib/ws-protocol'
import { lifecycleFromJobProgress, lifecycleFromJobStatus } from '@/lib/ws-protocol'
import {
  buildWorkspaceHandoffPrompt,
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

export function handleWsMessage(
  msg: WsServerMsg,
  refs: MessageHandlerRefs,
  setters: MessageHandlerSetters,
): void {
  const {
    currentModeRef,
    currentInputRef,
    currentJobIdRef,
    selectedFileRef,
    crawlFilesRef,
    stdoutJsonRef,
    currentOutputDirRef,
    virtualFileContentByPathRef,
    runIdCounter,
  } = refs
  const {
    setLogLines,
    setMarkdownContent,
    setHasResults,
    setCrawlFiles,
    setCurrentOutputDir,
    setSelectedFile,
    setCrawlProgress,
    setCommandMode,
    setStdoutLines,
    setStdoutJson,
    setVirtualFileContentByPath,
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
  } = setters

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
      setCurrentOutputDir(msg.output_dir)
      setHasResults(true)
      setCurrentJobIdTracked(msg.job_id ?? null)
      setSelectedFile((prev) =>
        prev && msg.files.some((file) => file.relative_path === prev) ? prev : null,
      )
      break
    case 'crawl_progress':
      setCrawlProgress(toCrawlProgress(msg))
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
      if (maybeJobData && typeof maybeJobData.output_dir === 'string') {
        setCurrentOutputDir(maybeJobData.output_dir)
      }
      if (msg.data.ctx.mode === 'scrape' && maybeJobData) {
        const markdown = typeof maybeJobData.markdown === 'string' ? maybeJobData.markdown : null
        const url =
          typeof maybeJobData.url === 'string' ? maybeJobData.url : currentInputRef.current
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
              {
                url,
                relative_path: relativePath,
                markdown_chars: markdown.length,
              },
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
      break
    }
    case 'command.output.line': {
      setStdoutLines((prev) => pushCapped(prev, msg.data.line))
      setHasResults(true)
      break
    }
    case 'job.status': {
      const lifecycle = lifecycleFromJobStatus(msg, currentJobIdRef.current)
      if (lifecycle) {
        setCurrentJobIdTracked(lifecycle.job_id)
        setLifecycleEntries((prev) => pushCapped(prev, lifecycle))
        setStdoutJson((prev) => pushCapped(prev, lifecycle))
      }
      setHasResults(true)
      break
    }
    case 'job.progress': {
      const lifecycle = lifecycleFromJobProgress(msg, currentJobIdRef.current)
      if (lifecycle) {
        setLifecycleEntries((prev) => pushCapped(prev, lifecycle))
        setStdoutJson((prev) => pushCapped(prev, lifecycle))
      }
      setHasResults(true)
      break
    }
    case 'artifact.list':
      setScreenshotFiles(toScreenshotFiles(msg.data.artifacts))
      setHasResults(true)
      break
    case 'artifact.content':
      if (
        (currentModeRef.current === 'scrape' ||
          currentModeRef.current === 'crawl' ||
          currentModeRef.current === 'extract') &&
        !selectedFileRef.current
      ) {
        break
      }
      setMarkdownContent(msg.data.content)
      setHasResults(true)
      break
    case 'job.cancel.response':
      setCancelResponse(toCancelResponse(msg.data.payload))
      setStatusResultLine(setLogLines, msg.data.payload.ok, msg.data.payload.message)
      break
    case 'stats':
      break
    case 'command.done': {
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
}
