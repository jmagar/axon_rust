'use client'

import { useCallback, useEffect, useState } from 'react'

export type PulseEffortLevel = 'low' | 'medium' | 'high'

export interface PulseSettings {
  effort: PulseEffortLevel
  maxTurns: number // 0 = unlimited
  maxBudgetUsd: number // 0 = unlimited
  appendSystemPrompt: string
  // Additional CLI flags wired from Claude Code docs
  disableSlashCommands: boolean // --disable-slash-commands
  noSessionPersistence: boolean // --no-session-persistence
  fallbackModel: string // --fallback-model ('' = disabled)
  allowedTools: string // --allowedTools ('' = all allowed)
  disallowedTools: string // --disallowedTools ('' = none disallowed)
  addDir: string // --add-dir (comma-separated directories)
  betas: string // --betas (comma-separated beta headers)
  toolsRestrict: string // --tools (restrict built-in tools)
  autoApprovePermissions: boolean // Show permission modal as informational-only overlay
}

const SETTINGS_KEY = 'axon.web.pulse.settings.v1'

export const DEFAULT_PULSE_SETTINGS: PulseSettings = {
  effort: 'medium',
  maxTurns: 0,
  maxBudgetUsd: 0,
  appendSystemPrompt: '',
  disableSlashCommands: false,
  noSessionPersistence: false,
  fallbackModel: '',
  allowedTools: '',
  disallowedTools: '',
  addDir: '',
  betas: '',
  toolsRestrict: '',
  autoApprovePermissions: true,
}

export function usePulseSettings() {
  const [settings, setSettings] = useState<PulseSettings>(DEFAULT_PULSE_SETTINGS)

  useEffect(() => {
    try {
      const raw = window.localStorage.getItem(SETTINGS_KEY)
      if (!raw) return
      const parsed = JSON.parse(raw) as Partial<PulseSettings>
      setSettings((prev) => ({
        ...prev,
        effort:
          parsed.effort === 'low' || parsed.effort === 'medium' || parsed.effort === 'high'
            ? parsed.effort
            : prev.effort,
        maxTurns:
          typeof parsed.maxTurns === 'number' && parsed.maxTurns >= 0
            ? parsed.maxTurns
            : prev.maxTurns,
        maxBudgetUsd:
          typeof parsed.maxBudgetUsd === 'number' && parsed.maxBudgetUsd >= 0
            ? parsed.maxBudgetUsd
            : prev.maxBudgetUsd,
        appendSystemPrompt:
          typeof parsed.appendSystemPrompt === 'string'
            ? parsed.appendSystemPrompt
            : prev.appendSystemPrompt,
        disableSlashCommands:
          typeof parsed.disableSlashCommands === 'boolean'
            ? parsed.disableSlashCommands
            : prev.disableSlashCommands,
        noSessionPersistence:
          typeof parsed.noSessionPersistence === 'boolean'
            ? parsed.noSessionPersistence
            : prev.noSessionPersistence,
        fallbackModel:
          typeof parsed.fallbackModel === 'string' ? parsed.fallbackModel : prev.fallbackModel,
        allowedTools:
          typeof parsed.allowedTools === 'string' ? parsed.allowedTools : prev.allowedTools,
        disallowedTools:
          typeof parsed.disallowedTools === 'string'
            ? parsed.disallowedTools
            : prev.disallowedTools,
        addDir: typeof parsed.addDir === 'string' ? parsed.addDir : prev.addDir,
        betas: typeof parsed.betas === 'string' ? parsed.betas : prev.betas,
        toolsRestrict:
          typeof parsed.toolsRestrict === 'string' ? parsed.toolsRestrict : prev.toolsRestrict,
        autoApprovePermissions:
          typeof parsed.autoApprovePermissions === 'boolean'
            ? parsed.autoApprovePermissions
            : prev.autoApprovePermissions,
      }))
    } catch {
      // Ignore storage errors.
    }
  }, [])

  const updateSettings = useCallback((patch: Partial<PulseSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...patch }
      try {
        window.localStorage.setItem(SETTINGS_KEY, JSON.stringify(next))
      } catch {
        // Ignore storage errors.
      }
      return next
    })
  }, [])

  return { settings, updateSettings }
}
