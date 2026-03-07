import type {
  AcpConfigOption,
  PulseAgent,
  PulseModel,
  PulsePermissionLevel,
} from '@/lib/pulse/types'
import type { WsLifecycleEntry, WsServerMsg } from '@/lib/ws-protocol'

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

export interface WorkspaceContextState {
  turns: number
  sourceCount: number
  threadSourceCount: number
  contextCharsTotal: number
  contextBudgetChars: number
  lastLatencyMs: number
  agent: PulseWorkspaceAgent
  model: PulseWorkspaceModel
  permissionLevel: 'plan' | 'accept-edits' | 'bypass-permissions'
  saveStatus?: 'idle' | 'saving' | 'saved' | 'error'
}

/** @deprecated Use PulseModel from @/lib/pulse/types directly */
export type PulseWorkspaceModel = PulseModel
/** @deprecated Use PulsePermissionLevel from @/lib/pulse/types directly */
export type PulseWorkspacePermission = PulsePermissionLevel
/** @deprecated Use PulseAgent from @/lib/pulse/types directly */
export type PulseWorkspaceAgent = PulseAgent

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

export interface RuntimeHandoffSnapshot {
  modeLabel: string
  targetInput: string
  filesSnapshot: Array<{ relative_path: string; markdown_chars: number; url: string }>
  outputDir: string | null
  stdoutSnapshot: unknown[]
  virtualFileContentByPath: Record<string, string>
}

export interface RuntimeHandoffResult {
  handoffPrompt: string
  hasResults: boolean
  workspaceMode: 'pulse'
}

export interface WsMessagesContextValue {
  markdownContent: string
  logLines: LogLine[]
  errorMessage: string
  recentRuns: RecentRun[]
  isProcessing: boolean
  hasResults: boolean
  currentMode: string
  crawlFiles: Array<{
    url: string
    relative_path: string
    markdown_chars: number
  }>
  selectedFile: string | null
  selectFile: (relativePath: string) => void
  crawlProgress: CrawlProgress | null
  stdoutLines: string[]
  stdoutJson: unknown[]
  commandMode: string | null
  screenshotFiles: ScreenshotFile[]
  currentJobId: string | null
  lifecycleEntries: WsLifecycleEntry[]
  cancelResponse: CancelResponseState | null
  workspaceMode: string | null
  workspacePrompt: string | null
  workspacePromptVersion: number
  workspaceResumeSessionId: string | null
  workspaceResumeVersion: number
  workspaceContext: WorkspaceContextState | null
  pulseModel: PulseWorkspaceModel
  pulsePermissionLevel: PulseWorkspacePermission
  acpConfigOptions: AcpConfigOption[]
  pulseAgent: PulseWorkspaceAgent
  setPulseAgent: (agent: PulseWorkspaceAgent) => void
  setPulseModel: (model: PulseWorkspaceModel) => void
  setPulsePermissionLevel: (level: PulseWorkspacePermission) => void
  setAcpConfigOptions: (options: AcpConfigOption[]) => void
  activateWorkspace: (mode: string) => void
  submitWorkspacePrompt: (prompt: string) => void
  resumeWorkspaceSession: (sessionId: string) => void
  clearWorkspaceResumeSession: () => void
  deactivateWorkspace: () => void
  updateWorkspaceContext: (context: WorkspaceContextState | null) => void
  startExecution: (mode: string, input?: string, options?: { preserveWorkspace?: boolean }) => void
}

export interface WsMessagesExecutionState {
  markdownContent: WsMessagesContextValue['markdownContent']
  logLines: WsMessagesContextValue['logLines']
  errorMessage: WsMessagesContextValue['errorMessage']
  recentRuns: WsMessagesContextValue['recentRuns']
  isProcessing: WsMessagesContextValue['isProcessing']
  hasResults: WsMessagesContextValue['hasResults']
  currentMode: WsMessagesContextValue['currentMode']
  crawlFiles: WsMessagesContextValue['crawlFiles']
  selectedFile: WsMessagesContextValue['selectedFile']
  crawlProgress: WsMessagesContextValue['crawlProgress']
  stdoutLines: WsMessagesContextValue['stdoutLines']
  stdoutJson: WsMessagesContextValue['stdoutJson']
  commandMode: WsMessagesContextValue['commandMode']
  screenshotFiles: WsMessagesContextValue['screenshotFiles']
  currentJobId: WsMessagesContextValue['currentJobId']
  lifecycleEntries: WsMessagesContextValue['lifecycleEntries']
  cancelResponse: WsMessagesContextValue['cancelResponse']
}

export interface WsMessagesWorkspaceState {
  workspaceMode: WsMessagesContextValue['workspaceMode']
  workspacePrompt: WsMessagesContextValue['workspacePrompt']
  workspacePromptVersion: WsMessagesContextValue['workspacePromptVersion']
  workspaceResumeSessionId: WsMessagesContextValue['workspaceResumeSessionId']
  workspaceResumeVersion: WsMessagesContextValue['workspaceResumeVersion']
  workspaceContext: WsMessagesContextValue['workspaceContext']
  pulseModel: WsMessagesContextValue['pulseModel']
  pulsePermissionLevel: WsMessagesContextValue['pulsePermissionLevel']
  pulseAgent: WsMessagesContextValue['pulseAgent']
  acpConfigOptions: WsMessagesContextValue['acpConfigOptions']
}

export interface WsMessagesActions {
  selectFile: WsMessagesContextValue['selectFile']
  setPulseAgent: WsMessagesContextValue['setPulseAgent']
  setPulseModel: WsMessagesContextValue['setPulseModel']
  setPulsePermissionLevel: WsMessagesContextValue['setPulsePermissionLevel']
  setAcpConfigOptions: WsMessagesContextValue['setAcpConfigOptions']
  activateWorkspace: WsMessagesContextValue['activateWorkspace']
  submitWorkspacePrompt: WsMessagesContextValue['submitWorkspacePrompt']
  resumeWorkspaceSession: WsMessagesContextValue['resumeWorkspaceSession']
  clearWorkspaceResumeSession: WsMessagesContextValue['clearWorkspaceResumeSession']
  deactivateWorkspace: WsMessagesContextValue['deactivateWorkspace']
  updateWorkspaceContext: WsMessagesContextValue['updateWorkspaceContext']
  startExecution: WsMessagesContextValue['startExecution']
}

export interface WsMessageRuntimeMappers {
  toProgress: (msg: Extract<WsServerMsg, { type: 'crawl_progress' }>) => CrawlProgress
}
