/**
 * Pulse workspace persistence — pure helpers with zero React imports.
 * Owns: ChatMessage interface, persisted state shape, localStorage key,
 * serialise/deserialise helpers, and the clamp utility.
 */

import type {
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

export type DesktopViewMode = 'chat' | 'editor' | 'both'
export type DesktopPaneOrder = 'editor-first' | 'chat-first'

export type PersistedPulseWorkspaceState = {
  permissionLevel: PulsePermissionLevel
  model: PulseModel
  documentMarkdown: string
  chatHistory: ChatMessage[]
  documentTitle: string
  chatSessionId: string | null
  indexedSources: string[]
  activeThreadSources: string[]
  desktopSplitPercent: number
  mobileSplitPercent: number
  lastResponseLatencyMs: number | null
  lastResponseModel: PulseModel | null
  desktopViewMode: DesktopViewMode
  desktopPaneOrder: DesktopPaneOrder
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
  return isNaN(n) ? def : n
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
    const model: PulseModel =
      parsed.model === 'opus' || parsed.model === 'haiku' || parsed.model === 'sonnet'
        ? parsed.model
        : 'sonnet'
    const permissionLevel: PulsePermissionLevel =
      parsed.permissionLevel === 'plan' ||
      parsed.permissionLevel === 'accept-edits' ||
      parsed.permissionLevel === 'bypass-permissions'
        ? parsed.permissionLevel
        : 'accept-edits'
    return {
      permissionLevel,
      model,
      documentMarkdown: parsed.documentMarkdown,
      chatHistory: Array.isArray(parsed.chatHistory) ? parsed.chatHistory.slice(-250) : [],
      documentTitle: parsed.documentTitle,
      chatSessionId: typeof parsed.chatSessionId === 'string' ? parsed.chatSessionId : null,
      indexedSources: Array.isArray(parsed.indexedSources) ? parsed.indexedSources.slice(-50) : [],
      activeThreadSources: Array.isArray(parsed.activeThreadSources)
        ? parsed.activeThreadSources.slice(-50)
        : [],
      desktopSplitPercent: clampSplit(parseSplit(parsed.desktopSplitPercent, 62), 42, 74),
      mobileSplitPercent: clampSplit(parseSplit(parsed.mobileSplitPercent, 56), 35, 70),
      lastResponseLatencyMs:
        typeof parsed.lastResponseLatencyMs === 'number' ? parsed.lastResponseLatencyMs : null,
      lastResponseModel:
        parsed.lastResponseModel === 'sonnet' ||
        parsed.lastResponseModel === 'opus' ||
        parsed.lastResponseModel === 'haiku'
          ? parsed.lastResponseModel
          : null,
      desktopViewMode:
        parsed.desktopViewMode === 'chat' ||
        parsed.desktopViewMode === 'editor' ||
        parsed.desktopViewMode === 'both'
          ? parsed.desktopViewMode
          : 'both',
      desktopPaneOrder: parsed.desktopPaneOrder === 'chat-first' ? 'chat-first' : 'editor-first',
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
