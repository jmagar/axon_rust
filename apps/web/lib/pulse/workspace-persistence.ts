/**
 * Pulse workspace persistence — pure helpers with zero React imports.
 * Owns: ChatMessage interface, persisted state shape, localStorage key,
 * serialise/deserialise helpers, and the clamp utility.
 */

import type {
  PulseAgent,
  PulseChatResponse,
  PulseMessageBlock,
  PulseModel,
  PulsePermissionLevel,
  PulseToolUse,
} from '@/lib/pulse/types'

// ── Types ─────────────────────────────────────────────────────────────────────

export interface ChatMessage {
  id?: string
  role: 'user' | 'assistant'
  content: string
  createdAt?: number
  citations?: PulseChatResponse['citations']
  operations?: PulseChatResponse['operations']
  toolUses?: PulseToolUse[]
  blocks?: PulseMessageBlock[]
  isError?: boolean
  retryPrompt?: string
}

export type PersistedPulseWorkspaceState = {
  permissionLevel: PulsePermissionLevel
  agent: PulseAgent
  model: PulseModel
  documentMarkdown: string
  chatHistory: ChatMessage[]
  documentTitle: string
  currentDocFilename: string | null
  chatSessionId: string | null
  indexedSources: string[]
  activeThreadSources: string[]
  desktopSplitPercent: number
  mobileSplitPercent: number
  lastResponseLatencyMs: number | null
  lastResponseModel: PulseModel | null
  showChat: boolean
  showEditor: boolean
  savedAt: number
}

// ── Constants ─────────────────────────────────────────────────────────────────

export const PULSE_WORKSPACE_STATE_KEY = 'axon.web.pulse.workspace-state.v2'

// ── Pure helpers ──────────────────────────────────────────────────────────────

export function clampSplit(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value))
}

function parseSplit(v: unknown, def: number): number {
  const n = Number(v ?? def)
  return Number.isNaN(n) ? def : n
}

export function parsePersistedWorkspaceState(
  raw: string | null,
): PersistedPulseWorkspaceState | null {
  if (!raw) return null
  try {
    const parsed = JSON.parse(raw) as Partial<PersistedPulseWorkspaceState>
    if (!parsed || typeof parsed !== 'object') return null
    if (typeof parsed.documentTitle !== 'string' || typeof parsed.documentMarkdown !== 'string') {
      return null
    }
    const agent: PulseAgent =
      parsed.agent === 'codex' || parsed.agent === 'claude' ? parsed.agent : 'claude'
    const model: PulseModel =
      parsed.model === 'opus' || parsed.model === 'haiku' || parsed.model === 'sonnet'
        ? parsed.model
        : 'sonnet'
    const permissionLevel: PulsePermissionLevel =
      parsed.permissionLevel === 'plan' ||
      parsed.permissionLevel === 'accept-edits' ||
      parsed.permissionLevel === 'bypass-permissions'
        ? parsed.permissionLevel
        : 'bypass-permissions'
    // Migration: if old desktopViewMode is present, derive showChat/showEditor from it.
    // New fields take priority.
    let showChat =
      typeof parsed.showChat === 'boolean'
        ? parsed.showChat
        : (parsed as Record<string, unknown>).desktopViewMode !== 'editor' // old: 'chat' or 'both' → showChat true
    const showEditor =
      typeof parsed.showEditor === 'boolean'
        ? parsed.showEditor
        : (parsed as Record<string, unknown>).desktopViewMode !== 'chat' // old: 'editor' or 'both' → showEditor true
    // Safety: never allow both panels to be collapsed on restore (stale blob guard)
    if (!showChat && !showEditor) showChat = true
    return {
      permissionLevel,
      agent,
      model,
      documentMarkdown: parsed.documentMarkdown,
      chatHistory: Array.isArray(parsed.chatHistory) ? parsed.chatHistory.slice(-250) : [],
      documentTitle: parsed.documentTitle,
      currentDocFilename:
        typeof parsed.currentDocFilename === 'string' ? parsed.currentDocFilename : null,
      chatSessionId: typeof parsed.chatSessionId === 'string' ? parsed.chatSessionId : null,
      indexedSources: Array.isArray(parsed.indexedSources) ? parsed.indexedSources.slice(-50) : [],
      activeThreadSources: Array.isArray(parsed.activeThreadSources)
        ? parsed.activeThreadSources.slice(-50)
        : [],
      desktopSplitPercent: clampSplit(parseSplit(parsed.desktopSplitPercent, 62), 20, 80),
      mobileSplitPercent: clampSplit(parseSplit(parsed.mobileSplitPercent, 56), 35, 70),
      lastResponseLatencyMs:
        typeof parsed.lastResponseLatencyMs === 'number' ? parsed.lastResponseLatencyMs : null,
      lastResponseModel:
        parsed.lastResponseModel === 'sonnet' ||
        parsed.lastResponseModel === 'opus' ||
        parsed.lastResponseModel === 'haiku'
          ? parsed.lastResponseModel
          : null,
      showChat,
      showEditor,
      savedAt: typeof parsed.savedAt === 'number' ? parsed.savedAt : Date.now(),
    }
  } catch {
    return null
  }
}

export function buildPersistedPayload(
  state: Omit<PersistedPulseWorkspaceState, 'savedAt'>,
): PersistedPulseWorkspaceState {
  return {
    ...state,
    chatHistory: state.chatHistory.slice(-250),
    indexedSources: state.indexedSources.slice(-50),
    activeThreadSources: state.activeThreadSources.slice(-50),
    savedAt: Date.now(),
  }
}
