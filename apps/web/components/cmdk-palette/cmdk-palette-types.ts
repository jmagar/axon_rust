import type { ModeDefinition } from '@/lib/ws-protocol'

export type PalettePhase = 'idle' | 'select' | 'input' | 'running' | 'done'

export interface PaletteProgress {
  phase: string
  percent?: number
  processed?: number
  total?: number
}

export interface PaletteDialogState {
  phase: PalettePhase
  search: string
  selectedMode: ModeDefinition | null
  inputValue: string
  lines: string[]
  jsonCount: number
  capturedJson: unknown[]
  progress: PaletteProgress | null
  exitCode: number | null
  errorMsg: string | null
  elapsedMs: number | null
  jobId: string | null
}
