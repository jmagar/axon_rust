'use client'

import { Brain, Cpu, Gauge, Info, Shield, Sparkles, Terminal, Wrench, Zap } from 'lucide-react'
import type { PulseSettings } from '@/hooks/use-pulse-settings'
import { getAcpModelConfigOption } from '@/lib/pulse/acp-config'
import type {
  AcpConfigOption,
  PulseAgent,
  PulseModel,
  PulsePermissionLevel,
} from '@/lib/pulse/types'
import {
  FieldHint,
  GLASS_SELECT,
  SectionDivider,
  SectionHeader,
  TextInput,
  ToggleRow,
} from './settings-components'
import {
  CLAUDE_MODEL_OPTIONS,
  EFFORT_OPTIONS,
  FALLBACK_MODEL_OPTIONS,
  KEYBOARD_SHORTCUTS,
  PERMISSION_OPTIONS,
} from './settings-data'

interface SettingsSectionsProps {
  pulseAgent: PulseAgent
  pulseModel: PulseModel
  acpConfigOptions: AcpConfigOption[]
  setPulseModel: (v: PulseModel) => void
  pulsePermissionLevel: PulsePermissionLevel
  setPulsePermissionLevel: (v: PulsePermissionLevel) => void
  settings: PulseSettings
  updateSettings: (patch: Partial<PulseSettings>) => void
}

export function SettingsSections({
  pulseAgent,
  pulseModel,
  acpConfigOptions,
  setPulseModel,
  pulsePermissionLevel,
  setPulsePermissionLevel,
  settings,
  updateSettings,
}: SettingsSectionsProps) {
  const acpModelOptions =
    getAcpModelConfigOption(acpConfigOptions)
      ?.options.map((option) => ({
        id: option.value,
        label: option.name,
        sub: option.description ?? '',
      }))
      .filter((o) => o.id) ?? []
  const modelOptions: Array<{ id: string; label: string; sub: string; badge?: string }> =
    pulseAgent === 'claude'
      ? CLAUDE_MODEL_OPTIONS.map((option) => ({
          id: option.id,
          label: option.label,
          sub: option.sub,
          badge: option.badge,
        }))
      : acpModelOptions.length > 0
        ? acpModelOptions
        : [{ id: 'default', label: 'Default', sub: 'Agent default model' }]
  const selectedModel = modelOptions.find((option) => option.id === pulseModel) ?? modelOptions[0]
  const selectedPermission =
    PERMISSION_OPTIONS.find((o) => o.id === pulsePermissionLevel) ?? PERMISSION_OPTIONS[0]
  const selectedEffort = EFFORT_OPTIONS.find((o) => o.id === settings.effort) ?? EFFORT_OPTIONS[1]

  return (
    <>
      {/* Model */}
      <section id="settings-section-model" className="scroll-mt-20">
        <div className="border-l-2 border-l-[var(--border-accent)] pl-3">
          <SectionHeader
            icon={Cpu}
            label="Model"
            description="The Claude model used for all Pulse chat sessions. Passed as --model to the Claude CLI."
          />
        </div>
        <select
          value={pulseModel}
          onChange={(e) => setPulseModel(e.target.value as PulseModel)}
          className={GLASS_SELECT}
          style={{ backdropFilter: 'blur(4px)' }}
        >
          {modelOptions.map((opt) => (
            <option key={opt.id} value={opt.id}>
              {opt.label}
              {opt.badge ? ` (${opt.badge})` : ''} — {opt.sub}
            </option>
          ))}
        </select>
        {selectedModel && (
          <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--text-dim)]">
            {selectedModel.sub}
            {selectedModel.badge && (
              <span className="ml-1.5 rounded-full border border-[rgba(175,215,255,0.2)] bg-[rgba(175,215,255,0.07)] px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wider text-[rgba(175,215,255,0.5)]">
                {selectedModel.badge}
              </span>
            )}
          </p>
        )}
      </section>

      <SectionDivider />

      {/* Permission Mode */}
      <section id="settings-section-permission" className="scroll-mt-20">
        <div className="border-l-2 border-l-[var(--border-accent)] pl-3">
          <SectionHeader
            icon={Shield}
            label="Permission Mode"
            description="Controls how Claude interacts with your filesystem and shell. Passed as --permission-mode to the Claude CLI."
          />
        </div>
        <select
          value={pulsePermissionLevel}
          onChange={(e) => setPulsePermissionLevel(e.target.value as PulsePermissionLevel)}
          className={GLASS_SELECT}
          style={{ backdropFilter: 'blur(4px)' }}
        >
          {PERMISSION_OPTIONS.map((opt) => (
            <option key={opt.id} value={opt.id}>
              {opt.label} — {opt.sub}
            </option>
          ))}
        </select>
        {selectedPermission && (
          <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--text-dim)]">
            {selectedPermission.sub}
          </p>
        )}
      </section>

      <SectionDivider />

      {/* Reasoning Effort */}
      <section id="settings-section-effort" className="scroll-mt-20">
        <div className="border-l-2 border-l-[var(--border-accent)] pl-3">
          <SectionHeader
            icon={Brain}
            label="Reasoning Effort"
            description="Controls how much thinking budget Claude uses per response. Passed as --effort to the Claude CLI."
          />
        </div>
        <select
          value={settings.effort}
          onChange={(e) => updateSettings({ effort: e.target.value as 'low' | 'medium' | 'high' })}
          className={GLASS_SELECT}
          style={{ backdropFilter: 'blur(4px)' }}
        >
          {EFFORT_OPTIONS.map((opt) => (
            <option key={opt.id} value={opt.id}>
              {opt.label} ({opt.hint}) — {opt.sub}
            </option>
          ))}
        </select>
        {selectedEffort && (
          <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--text-dim)]">
            {selectedEffort.hint} — {selectedEffort.sub}
          </p>
        )}
      </section>

      <SectionDivider />

      {/* Limits */}
      <section id="settings-section-limits" className="scroll-mt-20">
        <div className="border-l-2 border-l-[var(--border-accent)] pl-3">
          <SectionHeader
            icon={Gauge}
            label="Limits"
            description="Hard caps on agentic run length and API spend. 0 means unlimited (CLI default)."
          />
        </div>
        <div className="grid gap-5 sm:grid-cols-2">
          <div>
            <label
              htmlFor="settings-max-turns"
              className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--text-dim)]"
            >
              Max turns
              <code className="ml-1.5 normal-case tracking-normal text-[var(--text-dim)]">
                --max-turns
              </code>
            </label>
            <input
              id="settings-max-turns"
              type="number"
              min={0}
              max={200}
              value={settings.maxTurns}
              onChange={(e) =>
                updateSettings({
                  maxTurns: Math.max(0, Math.min(200, Number(e.target.value))),
                })
              }
              className="w-full rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.65)] px-3 py-2.5 text-[13px] text-[var(--text-secondary)] outline-none placeholder:text-[var(--text-dim)] focus:border-[var(--focus-ring-color)] focus:bg-[rgba(10,18,35,0.82)] transition-all duration-200"
              placeholder="0 (unlimited)"
              style={{ backdropFilter: 'blur(4px)' }}
            />
            <FieldHint>
              Maximum agentic loop iterations. Exits with an error when reached.
            </FieldHint>
          </div>
          <div>
            <label
              htmlFor="settings-max-budget"
              className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--text-dim)]"
            >
              Max budget USD
              <code className="ml-1.5 normal-case tracking-normal text-[var(--text-dim)]">
                --max-budget-usd
              </code>
            </label>
            <input
              id="settings-max-budget"
              type="number"
              min={0}
              max={1000}
              step={0.5}
              value={settings.maxBudgetUsd}
              onChange={(e) =>
                updateSettings({
                  maxBudgetUsd: Math.max(0, Math.min(1000, Number(e.target.value))),
                })
              }
              className="w-full rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.65)] px-3 py-2.5 text-[13px] text-[var(--text-secondary)] outline-none placeholder:text-[var(--text-dim)] focus:border-[var(--focus-ring-color)] focus:bg-[rgba(10,18,35,0.82)] transition-all duration-200"
              placeholder="0 (unlimited)"
              style={{ backdropFilter: 'blur(4px)' }}
            />
            <FieldHint>Stop before this dollar threshold is exceeded.</FieldHint>
          </div>
        </div>
      </section>

      <SectionDivider />

      {/* Custom Instructions */}
      <section id="settings-section-instructions" className="scroll-mt-20">
        <div className="border-l-2 border-l-[var(--border-accent)] pl-3">
          <SectionHeader
            icon={Sparkles}
            label="Custom Instructions"
            description="Appended to the system prompt on every Pulse request via --append-system-prompt. Adds rules without replacing Claude's built-in behavior."
          />
        </div>
        <textarea
          id="settings-append-system-prompt"
          value={settings.appendSystemPrompt}
          onChange={(e) => updateSettings({ appendSystemPrompt: e.target.value })}
          placeholder="e.g. Always respond in bullet points. Prefer TypeScript. Be concise."
          rows={5}
          maxLength={4000}
          className="w-full resize-none rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.65)] px-3 py-2.5 text-[13px] leading-relaxed text-[var(--text-secondary)] outline-none placeholder:text-[var(--text-dim)] focus:border-[var(--focus-ring-color)] focus:bg-[rgba(10,18,35,0.82)] transition-all duration-200"
          style={{ backdropFilter: 'blur(4px)' }}
        />
        <div className="mt-1.5 flex justify-between text-[10px] text-[var(--text-dim)]">
          <span>Applied to every Pulse chat request</span>
          <span>{settings.appendSystemPrompt.length} / 4000</span>
        </div>
      </section>

      <SectionDivider />

      {/* Tools & Permissions */}
      <section id="settings-section-tools" className="scroll-mt-20">
        <div className="border-l-2 border-l-[var(--border-accent)] pl-3">
          <SectionHeader
            icon={Wrench}
            label="Tools & Permissions"
            description="Fine-grained control over which tools Claude can use. Supports permission rule syntax (e.g. Bash(git log *), Read, Edit)."
          />
        </div>
        <div className="space-y-5">
          <div>
            <label
              htmlFor="settings-allowed-tools"
              className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--text-dim)]"
            >
              Allowed tools
              <code className="ml-1.5 normal-case tracking-normal text-[var(--text-dim)]">
                --allowedTools
              </code>
            </label>
            <TextInput
              id="settings-allowed-tools"
              value={settings.allowedTools}
              onChange={(v) => updateSettings({ allowedTools: v })}
              placeholder="e.g. Bash(git log *),Read,Edit"
              mono
            />
            <FieldHint>
              Comma-separated tools that execute without prompting for permission. Leave blank for
              defaults.
            </FieldHint>
          </div>
          <div>
            <label
              htmlFor="settings-disallowed-tools"
              className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--text-dim)]"
            >
              Disallowed tools
              <code className="ml-1.5 normal-case tracking-normal text-[var(--text-dim)]">
                --disallowedTools
              </code>
            </label>
            <TextInput
              id="settings-disallowed-tools"
              value={settings.disallowedTools}
              onChange={(v) => updateSettings({ disallowedTools: v })}
              placeholder="e.g. Bash,Edit"
              mono
            />
            <FieldHint>
              Comma-separated tools removed from Claude's context entirely. Takes priority over
              allowed tools.
            </FieldHint>
          </div>
          <div>
            <label
              htmlFor="settings-tools-restrict"
              className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--text-dim)]"
            >
              Restrict built-in tools
              <code className="ml-1.5 normal-case tracking-normal text-[var(--text-dim)]">
                --tools
              </code>
            </label>
            <TextInput
              id="settings-tools-restrict"
              value={settings.toolsRestrict}
              onChange={(v) => updateSettings({ toolsRestrict: v })}
              placeholder="e.g. Bash,Edit,Read"
              mono
            />
            <FieldHint>
              Restrict which built-in tools are available. Use empty string for all tools. Different
              from --allowedTools.
            </FieldHint>
          </div>

          <div
            className="flex items-start gap-2.5 rounded-lg border border-[rgba(175,215,255,0.12)] px-3.5 py-3 transition-all duration-200"
            style={{ background: 'rgba(10,18,35,0.58)', backdropFilter: 'blur(8px)' }}
          >
            <Info className="mt-0.5 size-3.5 shrink-0 text-[var(--axon-primary-strong)]" />
            <p className="text-[11px] leading-relaxed text-[var(--text-dim)]">
              Pulse always runs with{' '}
              <code className="rounded bg-[rgba(175,215,255,0.07)] px-1 py-0.5 font-mono text-[10px] text-[var(--text-muted)]">
                --dangerously-skip-permissions
              </code>{' '}
              because there is no TTY in the container environment. These tool filters layer on top
              and do not restore the interactive permission prompt.
            </p>
          </div>
        </div>
      </section>

      <SectionDivider />

      {/* Session Behavior */}
      <section id="settings-section-session" className="scroll-mt-20">
        <div className="border-l-2 border-l-[var(--border-accent)] pl-3">
          <SectionHeader
            icon={Terminal}
            label="Session Behavior"
            description="Control how the Claude CLI manages sessions and handles built-in commands during each chat."
          />
        </div>
        <div className="space-y-3">
          <ToggleRow
            id="settings-disable-slash-commands"
            label="Disable slash commands"
            description="Disables all skills and slash commands for each session."
            cliFlag="--disable-slash-commands"
            checked={settings.disableSlashCommands}
            onChange={(v) => updateSettings({ disableSlashCommands: v })}
          />
          <ToggleRow
            id="settings-no-session-persistence"
            label="Disable session persistence"
            description="Sessions are not saved to disk and cannot be resumed."
            cliFlag="--no-session-persistence"
            checked={settings.noSessionPersistence}
            onChange={(v) => updateSettings({ noSessionPersistence: v })}
          />
          <div>
            <label
              htmlFor="settings-fallback-model"
              className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--text-dim)]"
            >
              Fallback model
              <code className="ml-1.5 normal-case tracking-normal text-[var(--text-dim)]">
                --fallback-model
              </code>
            </label>
            <select
              id="settings-fallback-model"
              value={settings.fallbackModel}
              onChange={(e) => updateSettings({ fallbackModel: e.target.value })}
              className={GLASS_SELECT}
              style={{ backdropFilter: 'blur(4px)' }}
            >
              {FALLBACK_MODEL_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
            <FieldHint>
              Automatically falls back to this model when the primary model is overloaded.
            </FieldHint>
          </div>
          <div>
            <label
              htmlFor="settings-add-dir"
              className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--text-dim)]"
            >
              Additional directories
              <code className="ml-1.5 normal-case tracking-normal text-[var(--text-dim)]">
                --add-dir
              </code>
            </label>
            <TextInput
              id="settings-add-dir"
              value={settings.addDir}
              onChange={(v) => updateSettings({ addDir: v })}
              placeholder="e.g. /home/user/docs,/tmp/scratch"
              mono
            />
            <FieldHint>
              Comma-separated directories for Claude to access beyond the working dir.
            </FieldHint>
          </div>
          <div>
            <label
              htmlFor="settings-betas"
              className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--text-dim)]"
            >
              Beta features
              <code className="ml-1.5 normal-case tracking-normal text-[var(--text-dim)]">
                --betas
              </code>
            </label>
            <TextInput
              id="settings-betas"
              value={settings.betas}
              onChange={(v) => updateSettings({ betas: v })}
              placeholder="e.g. interleaved-thinking"
              mono
            />
            <FieldHint>
              Comma-separated beta headers (e.g. interleaved-thinking). API key users only.
            </FieldHint>
          </div>
        </div>
      </section>

      <SectionDivider />

      {/* Keyboard Shortcuts */}
      <section id="settings-section-shortcuts" className="scroll-mt-20">
        <div className="border-l-2 border-l-[var(--border-accent)] pl-3">
          <SectionHeader
            icon={Zap}
            label="Keyboard Shortcuts"
            description="Global shortcuts available throughout the Pulse workspace and omnibox."
          />
        </div>
        <div
          className="overflow-hidden rounded-xl border border-[var(--border-subtle)]"
          style={{ background: 'rgba(10,18,35,0.58)', backdropFilter: 'blur(8px)' }}
        >
          {KEYBOARD_SHORTCUTS.map(({ keys, desc }, idx) => (
            <div
              key={desc}
              className={`flex items-center justify-between px-4 py-3 ${
                idx < KEYBOARD_SHORTCUTS.length - 1 ? 'border-b border-[var(--border-subtle)]' : ''
              }`}
            >
              <span className="text-[12px] text-[var(--text-dim)]">{desc}</span>
              <div className="flex items-center gap-1">
                {keys.map((k, ki) => (
                  <span key={k} className="flex items-center gap-1">
                    {ki > 0 && <span className="text-[10px] text-[var(--text-dim)]">+</span>}
                    <kbd className="rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.6)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--text-dim)]">
                      {k}
                    </kbd>
                  </span>
                ))}
              </div>
            </div>
          ))}
        </div>
      </section>

      {/* Bottom breathing room */}
      <div className="h-16" />
    </>
  )
}
