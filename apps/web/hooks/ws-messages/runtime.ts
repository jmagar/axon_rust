import type { Dispatch, SetStateAction } from 'react'
import {
  type CrawlFile,
  lifecycleFromJobProgress,
  lifecycleFromJobStatus,
  type WsServerMsg,
} from '@/lib/ws-protocol'
import type {
  CancelResponseState,
  CrawlProgress,
  LogLine,
  RuntimeHandoffSnapshot,
  ScreenshotFile,
  WsMessagesRuntimeState,
} from './types'

export const MAX_STDOUT_ITEMS = 5000
export const MAX_LOG_LINES = 5000

export function pushCapped<T>(items: T[], item: T, cap = MAX_STDOUT_ITEMS): T[] {
  if (items.length >= cap) {
    // Trim 10% when at cap to amortize the copy cost
    return items.slice(-Math.floor(cap * 0.9)).concat(item)
  }
  return items.concat(item)
}

function truncateText(input: string, maxChars: number): string {
  if (input.length <= maxChars) return input
  return `${input.slice(0, maxChars)}\n\n[truncated ${input.length - maxChars} chars]`
}

export function summarizeJsonValue(value: unknown): string {
  if (value == null) return 'null'
  if (typeof value === 'string') return truncateText(value, 1200)
  if (typeof value === 'number' || typeof value === 'boolean') return String(value)
  try {
    return truncateText(JSON.stringify(value, null, 2), 2400)
  } catch {
    return '[unserializable output]'
  }
}

export function toCrawlProgress(
  msg: Extract<WsServerMsg, { type: 'crawl_progress' }>,
): CrawlProgress {
  return {
    pages_crawled: msg.pages_crawled,
    pages_discovered: msg.pages_discovered,
    md_created: msg.md_created,
    thin_md: msg.thin_md,
    phase: msg.phase,
  }
}

export function toScreenshotFiles(
  artifacts: Extract<WsServerMsg, { type: 'artifact.list' }>['data']['artifacts'],
): ScreenshotFile[] {
  return artifacts
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
}

export function toCancelResponse(
  payload: Extract<WsServerMsg, { type: 'job.cancel.response' }>['data']['payload'],
): CancelResponseState {
  return {
    ok: payload.ok,
    message: payload.message ?? (payload.ok ? 'Cancel request accepted' : 'Cancel request failed'),
    mode: payload.mode,
    job_id: payload.job_id,
  }
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

/**
 * Pure reducer for runtime state — mirrors a subset of handleWsMessage logic.
 * Used by tests (ws-messages-runtime.test.ts, use-ws-messages.test.ts).
 * IMPORTANT: When updating message handling in handlers.ts, update the
 * matching cases here to prevent divergence.
 */
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
      next.screenshotFiles = toScreenshotFiles(msg.data.artifacts)
      return next
    case 'artifact.content':
      next.markdownContent = msg.data.content
      return next
    case 'job.cancel.response':
      next.cancelResponse = toCancelResponse(msg.data.payload)
      return next
    case 'crawl_progress':
      next.crawlProgress = toCrawlProgress(msg)
      return next
    default:
      return next
  }
}

export function setStatusResultLine(
  setLogLines: Dispatch<SetStateAction<LogLine[]>>,
  ok: boolean,
  message?: string,
) {
  const line = message ?? (ok ? 'Cancel request accepted' : 'Cancel request failed')
  setLogLines((prev) =>
    pushCapped(prev, { content: `[cancel] ${line}`, timestamp: Date.now() }, MAX_LOG_LINES),
  )
}

export function buildWorkspaceHandoffPrompt(snapshot: RuntimeHandoffSnapshot): string {
  const {
    filesSnapshot,
    modeLabel,
    outputDir,
    stdoutSnapshot,
    targetInput,
    virtualFileContentByPath,
  } = snapshot
  const summary =
    stdoutSnapshot.length > 0
      ? summarizeJsonValue(stdoutSnapshot[stdoutSnapshot.length - 1])
      : 'No JSON summary available.'

  if (modeLabel === 'scrape') {
    const scrapeFile =
      filesSnapshot.find((file) => file.relative_path.startsWith('virtual/scrape-')) ??
      filesSnapshot[0]
    const scrapeMarkdown =
      scrapeFile && virtualFileContentByPath[scrapeFile.relative_path]
        ? virtualFileContentByPath[scrapeFile.relative_path]
        : null
    return [
      `I just scraped: ${targetInput || scrapeFile?.url || 'unknown source'}.`,
      '',
      'Use this full scraped page as context for our conversation:',
      '',
      scrapeMarkdown ? scrapeMarkdown : '(No scrape markdown captured in-memory.)',
      '',
      scrapeFile
        ? `If you need to re-open the source, use file explorer item: ${scrapeFile.relative_path}.`
        : 'If you need source files, check the file explorer sidebar.',
    ].join('\n')
  }

  const listedFiles = filesSnapshot
    .slice(0, 20)
    .map((file: CrawlFile) => `- ${file.relative_path} (${file.markdown_chars} chars)`)
    .join('\n')

  return [
    `I just ran ${modeLabel} for: ${targetInput || 'current target'}.`,
    '',
    'Start by giving me a concise summary of what was collected.',
    'Then propose the top next questions/actions.',
    '',
    'Use these sidebar files for deeper details:',
    listedFiles || '- (No files listed yet)',
    outputDir ? `Base output directory: ${outputDir}` : 'Base output directory: (not provided)',
    '',
    'Execution summary payload:',
    summary,
  ].join('\n')
}
