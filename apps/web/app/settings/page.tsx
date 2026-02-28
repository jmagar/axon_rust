'use client'

import {
  ArrowLeft,
  Bot,
  Brain,
  Cpu,
  Gauge,
  Info,
  RotateCcw,
  Server,
  Shield,
  ShieldCheck,
  ShieldOff,
  SlidersHorizontal,
  Sparkles,
  Terminal,
  Wrench,
  Zap,
} from 'lucide-react'
import dynamic from 'next/dynamic'
import { useRouter } from 'next/navigation'
import { useEffect, useRef, useState } from 'react'
import type { NeuralCanvasHandle } from '@/components/neural-canvas'
import { DEFAULT_PULSE_SETTINGS, usePulseSettings } from '@/hooks/use-pulse-settings'
import { useWsMessages } from '@/hooks/use-ws-messages'
import type { PulseModel, PulsePermissionLevel } from '@/lib/pulse/types'

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), { ssr: false })

// ── Option data ───────────────────────────────────────────────────────────────

const MODEL_OPTIONS: { id: PulseModel; label: string; sub: string; badge?: string }[] = [
  {
    id: 'sonnet',
    label: 'Claude Sonnet 4.6',
    sub: 'Balanced intelligence and speed',
    badge: 'Default',
  },
  { id: 'opus', label: 'Claude Opus 4.6', sub: 'Most capable — best for complex tasks' },
  { id: 'haiku', label: 'Claude Haiku 4.5', sub: 'Fastest response — most efficient' },
]

const PERMISSION_OPTIONS: {
  id: PulsePermissionLevel
  label: string
  sub: string
  icon: React.ComponentType<{ className?: string }>
  accentColor: string
}[] = [
  {
    id: 'plan',
    label: 'Plan',
    sub: 'Read-only analysis — no file changes or commands executed',
    icon: Shield,
    accentColor: 'rgba(175,215,255,0.7)',
  },
  {
    id: 'accept-edits',
    label: 'Accept Edits',
    sub: 'Claude proposes changes; you confirm each edit before it applies',
    icon: ShieldCheck,
    accentColor: 'rgba(255,135,175,0.7)',
  },
  {
    id: 'bypass-permissions',
    label: 'Bypass Permissions',
    sub: 'Apply all changes directly without confirmation prompts',
    icon: ShieldOff,
    accentColor: 'rgba(255,175,100,0.7)',
  },
]

const EFFORT_OPTIONS: {
  id: 'low' | 'medium' | 'high'
  label: string
  hint: string
  sub: string
}[] = [
  { id: 'low', label: 'Low', hint: 'Fastest', sub: 'Quick answers, minimal reasoning' },
  { id: 'medium', label: 'Medium', hint: 'Balanced', sub: 'Default thinking budget' },
  { id: 'high', label: 'High', hint: 'Thorough', sub: 'Extended reasoning, deepest analysis' },
]

const FALLBACK_MODEL_OPTIONS: { value: string; label: string }[] = [
  { value: '', label: 'Disabled (no fallback)' },
  { value: 'sonnet', label: 'Sonnet' },
  { value: 'opus', label: 'Opus' },
  { value: 'haiku', label: 'Haiku' },
]

const KEYBOARD_SHORTCUTS = [
  { keys: ['/', 'Ctrl+K'], desc: 'Focus the omnibox' },
  { keys: ['Alt', '1'], desc: 'Switch to Sonnet' },
  { keys: ['Alt', '2'], desc: 'Switch to Opus' },
  { keys: ['Alt', '3'], desc: 'Switch to Haiku' },
  { keys: ['Alt', 'Shift', '1'], desc: 'Plan permission mode' },
  { keys: ['Alt', 'Shift', '2'], desc: 'Accept Edits mode' },
  { keys: ['Alt', 'Shift', '3'], desc: 'Bypass Permissions mode' },
]

const NAV_SECTIONS = [
  { id: 'model', label: 'Model', icon: Cpu },
  { id: 'permission', label: 'Permission Mode', icon: Shield },
  { id: 'effort', label: 'Reasoning Effort', icon: Brain },
  { id: 'limits', label: 'Limits', icon: Gauge },
  { id: 'instructions', label: 'Custom Instructions', icon: Sparkles },
  { id: 'tools', label: 'Tools & Permissions', icon: Wrench },
  { id: 'session', label: 'Session Behavior', icon: Terminal },
  { id: 'shortcuts', label: 'Keyboard Shortcuts', icon: Zap },
]

// ── Reusable sub-components ───────────────────────────────────────────────────

function SectionHeader({
  icon: Icon,
  label,
  description,
}: {
  icon: React.ComponentType<{ className?: string }>
  label: string
  description?: string
}) {
  return (
    <div className="mb-5">
      <div className="flex items-center gap-2.5">
        <div
          className="flex size-7 shrink-0 items-center justify-center rounded-md border border-[rgba(255,135,175,0.18)] bg-[rgba(255,135,175,0.07)]"
          style={{ boxShadow: '0 0 12px rgba(255,135,175,0.08)' }}
        >
          <Icon className="size-3.5 text-[var(--axon-accent-pink)]" />
        </div>
        <h2 className="text-sm font-semibold text-[var(--axon-text-primary)]">{label}</h2>
      </div>
      {description && (
        <p className="mt-1.5 pl-[2.375rem] text-[12px] leading-relaxed text-[var(--axon-text-dim)]">
          {description}
        </p>
      )}
    </div>
  )
}

function FieldHint({ children }: { children: React.ReactNode }) {
  return (
    <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--axon-text-dim)]">{children}</p>
  )
}

function SectionDivider() {
  return <div className="my-8 h-px bg-[rgba(255,135,175,0.07)]" />
}

function ToggleRow({
  id,
  label,
  description,
  cliFlag,
  checked,
  onChange,
}: {
  id: string
  label: string
  description: string
  cliFlag: string
  checked: boolean
  onChange: (v: boolean) => void
}) {
  return (
    <div
      className="flex items-start justify-between gap-4 rounded-xl border border-[rgba(255,135,175,0.1)] px-4 py-3.5 transition-all duration-200"
      style={{ background: 'rgba(10,18,35,0.58)', backdropFilter: 'blur(8px)' }}
    >
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-medium text-[var(--axon-text-secondary)]">{label}</p>
        <p className="mt-0.5 text-[11px] text-[var(--axon-text-dim)]">
          {description}{' '}
          <code className="rounded bg-[rgba(175,215,255,0.07)] px-1 py-0.5 font-mono text-[10px] text-[var(--axon-text-muted)]">
            {cliFlag}
          </code>
        </p>
      </div>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className="relative mt-0.5 inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-all duration-200 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgba(175,215,255,0.5)]"
        style={{ background: checked ? 'var(--axon-accent-pink)' : 'rgba(255,135,175,0.15)' }}
        aria-label={label}
      >
        <span
          className="inline-block size-3.5 rounded-full bg-white shadow-sm transition-transform duration-200"
          style={{ transform: checked ? 'translateX(18px)' : 'translateX(2px)' }}
        />
      </button>
    </div>
  )
}

function TextInput({
  id,
  value,
  onChange,
  placeholder,
  mono,
}: {
  id: string
  value: string
  onChange: (v: string) => void
  placeholder?: string
  mono?: boolean
}) {
  return (
    <input
      id={id}
      type="text"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      className={`w-full rounded-lg border border-[rgba(255,135,175,0.15)] bg-[rgba(10,18,35,0.65)] px-3 py-2.5 text-[13px] text-[var(--axon-text-secondary)] outline-none placeholder:text-[var(--axon-text-subtle)] focus:border-[rgba(175,215,255,0.35)] focus:bg-[rgba(10,18,35,0.82)] transition-all duration-200 ${mono ? 'font-mono' : ''}`}
      style={{ backdropFilter: 'blur(4px)' }}
    />
  )
}

const GLASS_SELECT =
  'w-full rounded-lg border border-[rgba(255,135,175,0.2)] bg-[rgba(10,18,35,0.65)] px-3 py-2.5 text-[13px] text-[var(--axon-text-secondary)] outline-none focus:border-[rgba(175,215,255,0.4)] focus:bg-[rgba(10,18,35,0.82)] cursor-pointer appearance-none transition-all duration-200'

// ── Page ──────────────────────────────────────────────────────────────────────

export default function SettingsPage() {
  const router = useRouter()
  const { pulseModel, pulsePermissionLevel, setPulseModel, setPulsePermissionLevel } =
    useWsMessages()
  const { settings, updateSettings } = usePulseSettings()
  const [activeSection, setActiveSection] = useState('model')
  const canvasRef = useRef<NeuralCanvasHandle>(null)
  const [resetConfirming, setResetConfirming] = useState(false)

  useEffect(() => {
    canvasRef.current?.setIntensity(0.06)
  }, [])

  useEffect(() => {
    if (!resetConfirming) return
    const t = setTimeout(() => setResetConfirming(false), 5000)
    return () => clearTimeout(t)
  }, [resetConfirming])

  function handleReset() {
    updateSettings(DEFAULT_PULSE_SETTINGS)
    setPulseModel('sonnet')
    setPulsePermissionLevel('accept-edits')
  }

  function scrollTo(id: string) {
    setActiveSection(id)
    const el = document.getElementById(`settings-section-${id}`)
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }

  const selectedModel = MODEL_OPTIONS.find((o) => o.id === pulseModel) ?? MODEL_OPTIONS[0]
  const selectedPermission =
    PERMISSION_OPTIONS.find((o) => o.id === pulsePermissionLevel) ?? PERMISSION_OPTIONS[0]
  const selectedEffort = EFFORT_OPTIONS.find((o) => o.id === settings.effort) ?? EFFORT_OPTIONS[1]

  return (
    <div className="flex min-h-dvh flex-col">
      {/* NeuralCanvas background */}
      <div className="pointer-events-none fixed inset-0 z-0">
        <NeuralCanvas ref={canvasRef} />
      </div>

      {/* All page content above canvas */}
      <div className="relative z-[1] flex min-h-dvh flex-col">
        {/* Top bar */}
        <header
          className="sticky top-0 z-30 flex h-13 shrink-0 items-center gap-3 border-b px-4"
          style={{
            borderColor: 'rgba(255,135,175,0.1)',
            background: 'rgba(3,7,18,0.86)',
            backdropFilter: 'blur(20px) saturate(180%)',
            height: '3.25rem',
          }}
        >
          <button
            type="button"
            onClick={() => router.back()}
            className="flex items-center gap-1.5 rounded-md px-2 py-1 text-[12px] font-medium text-[var(--axon-text-dim)] transition-all duration-200 hover:bg-[rgba(255,135,175,0.08)] hover:text-[var(--axon-text-secondary)]"
            aria-label="Go back"
          >
            <ArrowLeft className="size-3.5" />
            Back
          </button>
          <div className="h-4 w-px bg-[rgba(255,135,175,0.12)]" />
          <div className="flex items-center gap-2">
            <SlidersHorizontal className="size-3.5 text-[var(--axon-accent-pink)]" />
            <h1 className="text-[14px] font-semibold text-[var(--axon-text-primary)]">Settings</h1>
          </div>
          <div className="flex-1" />
          {resetConfirming ? (
            <>
              <button
                type="button"
                onClick={() => {
                  handleReset()
                  setResetConfirming(false)
                }}
                className="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-medium text-[var(--axon-accent-pink-strong)] transition-all duration-200 bg-[rgba(255,135,175,0.12)] hover:bg-[rgba(255,135,175,0.2)]"
              >
                Confirm Reset?
              </button>
              <button
                type="button"
                onClick={() => setResetConfirming(false)}
                className="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-medium text-[var(--axon-text-dim)] transition-all duration-200 hover:bg-[rgba(255,135,175,0.08)] hover:text-[var(--axon-text-secondary)]"
              >
                Cancel
              </button>
            </>
          ) : (
            <button
              type="button"
              onClick={() => setResetConfirming(true)}
              className="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-medium text-[var(--axon-text-dim)] transition-all duration-200 hover:bg-[rgba(255,135,175,0.08)] hover:text-[var(--axon-accent-pink-strong)]"
              title="Reset all settings to defaults"
            >
              <RotateCcw className="size-3" />
              Reset to defaults
            </button>
          )}
        </header>

        {/* Body */}
        <div className="flex flex-1">
          {/* Sidebar nav — hidden below lg breakpoint */}
          <nav
            className="sticky hidden h-[calc(100vh-3.25rem)] w-52 shrink-0 flex-col gap-0.5 overflow-y-auto border-r p-3 lg:flex"
            style={{
              top: '3.25rem',
              borderColor: 'rgba(255,135,175,0.08)',
              background: 'rgba(3,7,18,0.70)',
              backdropFilter: 'blur(16px)',
            }}
          >
            <p className="mb-2 px-2.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--axon-text-dim)]">
              Configuration
            </p>
            {NAV_SECTIONS.map(({ id, label, icon: Icon }) => (
              <button
                key={id}
                type="button"
                onClick={() => scrollTo(id)}
                className={`flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[12px] font-medium transition-all duration-150 ${
                  activeSection === id
                    ? 'text-[var(--axon-accent-pink-strong)]'
                    : 'text-[var(--axon-text-muted)] hover:bg-[rgba(255,135,175,0.06)] hover:text-[var(--axon-text-secondary)]'
                }`}
                style={activeSection === id ? { background: 'rgba(255,135,175,0.12)' } : undefined}
              >
                <Icon className="size-3.5 shrink-0" />
                {label}
              </button>
            ))}

            {/* Related links */}
            <div className="mt-2 pt-3 border-t border-[rgba(255,135,175,0.07)]">
              <p className="mb-1.5 px-2.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--axon-text-dim)]">
                Related
              </p>
              <button
                type="button"
                onClick={() => router.push('/mcp')}
                className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[12px] font-medium text-[var(--axon-text-muted)] transition-all duration-150 hover:bg-[rgba(255,135,175,0.06)] hover:text-[var(--axon-text-secondary)]"
              >
                <Server className="size-3.5 shrink-0" />
                MCP Servers
              </button>
              <button
                type="button"
                onClick={() => router.push('/agents')}
                className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[12px] font-medium text-[var(--axon-text-muted)] transition-all duration-150 hover:bg-[rgba(255,135,175,0.06)] hover:text-[var(--axon-text-secondary)]"
              >
                <Bot className="size-3.5 shrink-0" />
                Agents
              </button>
            </div>

            <div className="mt-auto pt-4">
              {resetConfirming ? (
                <div className="flex flex-col gap-1">
                  <button
                    type="button"
                    onClick={() => {
                      handleReset()
                      setResetConfirming(false)
                    }}
                    className="flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-[11px] font-medium text-[var(--axon-accent-pink-strong)] transition-all duration-200 bg-[rgba(255,135,175,0.12)] hover:bg-[rgba(255,135,175,0.2)]"
                  >
                    Confirm Reset?
                  </button>
                  <button
                    type="button"
                    onClick={() => setResetConfirming(false)}
                    className="flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-[11px] font-medium text-[var(--axon-text-dim)] transition-all duration-200 hover:bg-[rgba(255,135,175,0.08)] hover:text-[var(--axon-text-secondary)]"
                  >
                    Cancel
                  </button>
                </div>
              ) : (
                <button
                  type="button"
                  onClick={() => setResetConfirming(true)}
                  className="flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-[11px] font-medium text-[var(--axon-text-dim)] transition-all duration-200 hover:bg-[rgba(255,135,175,0.08)] hover:text-[var(--axon-accent-pink-strong)]"
                >
                  <RotateCcw className="size-3 shrink-0" />
                  Reset all to defaults
                </button>
              )}
            </div>
          </nav>

          {/* Main content column */}
          <main className="flex-1 overflow-y-auto">
            <div className="mx-auto max-w-[720px] px-4 py-8 sm:px-6">
              {/* ── Model ─────────────────────────────────────────────── */}
              <section id="settings-section-model" className="scroll-mt-20">
                <SectionHeader
                  icon={Cpu}
                  label="Model"
                  description="The Claude model used for all Pulse chat sessions. Passed as --model to the Claude CLI."
                />
                <select
                  value={pulseModel}
                  onChange={(e) => setPulseModel(e.target.value as PulseModel)}
                  className={GLASS_SELECT}
                  style={{ backdropFilter: 'blur(4px)' }}
                >
                  {MODEL_OPTIONS.map((opt) => (
                    <option key={opt.id} value={opt.id}>
                      {opt.label}
                      {opt.badge ? ` (${opt.badge})` : ''} — {opt.sub}
                    </option>
                  ))}
                </select>
                {selectedModel && (
                  <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--axon-text-dim)]">
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

              {/* ── Permission Mode ───────────────────────────────────── */}
              <section id="settings-section-permission" className="scroll-mt-20">
                <SectionHeader
                  icon={Shield}
                  label="Permission Mode"
                  description="Controls how Claude interacts with your filesystem and shell. Passed as --permission-mode to the Claude CLI."
                />
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
                  <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--axon-text-dim)]">
                    {selectedPermission.sub}
                  </p>
                )}
              </section>

              <SectionDivider />

              {/* ── Reasoning Effort ──────────────────────────────────── */}
              <section id="settings-section-effort" className="scroll-mt-20">
                <SectionHeader
                  icon={Brain}
                  label="Reasoning Effort"
                  description="Controls how much thinking budget Claude uses per response. Passed as --effort to the Claude CLI."
                />
                <select
                  value={settings.effort}
                  onChange={(e) =>
                    updateSettings({ effort: e.target.value as 'low' | 'medium' | 'high' })
                  }
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
                  <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--axon-text-dim)]">
                    {selectedEffort.hint} — {selectedEffort.sub}
                  </p>
                )}
              </section>

              <SectionDivider />

              {/* ── Limits ───────────────────────────────────────────── */}
              <section id="settings-section-limits" className="scroll-mt-20">
                <SectionHeader
                  icon={Gauge}
                  label="Limits"
                  description="Hard caps on agentic run length and API spend. 0 means unlimited (CLI default)."
                />
                <div className="grid gap-5 sm:grid-cols-2">
                  <div>
                    <label
                      htmlFor="settings-max-turns"
                      className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--axon-text-dim)]"
                    >
                      Max turns
                      <code className="ml-1.5 normal-case tracking-normal text-[var(--axon-text-subtle)]">
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
                      className="w-full rounded-lg border border-[rgba(255,135,175,0.15)] bg-[rgba(10,18,35,0.65)] px-3 py-2.5 text-[13px] text-[var(--axon-text-secondary)] outline-none placeholder:text-[var(--axon-text-subtle)] focus:border-[rgba(175,215,255,0.35)] focus:bg-[rgba(10,18,35,0.82)] transition-all duration-200"
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
                      className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--axon-text-dim)]"
                    >
                      Max budget USD
                      <code className="ml-1.5 normal-case tracking-normal text-[var(--axon-text-subtle)]">
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
                      className="w-full rounded-lg border border-[rgba(255,135,175,0.15)] bg-[rgba(10,18,35,0.65)] px-3 py-2.5 text-[13px] text-[var(--axon-text-secondary)] outline-none placeholder:text-[var(--axon-text-subtle)] focus:border-[rgba(175,215,255,0.35)] focus:bg-[rgba(10,18,35,0.82)] transition-all duration-200"
                      placeholder="0 (unlimited)"
                      style={{ backdropFilter: 'blur(4px)' }}
                    />
                    <FieldHint>Stop before this dollar threshold is exceeded.</FieldHint>
                  </div>
                </div>
              </section>

              <SectionDivider />

              {/* ── Custom Instructions ──────────────────────────────── */}
              <section id="settings-section-instructions" className="scroll-mt-20">
                <SectionHeader
                  icon={Sparkles}
                  label="Custom Instructions"
                  description="Appended to the system prompt on every Pulse request via --append-system-prompt. Adds rules without replacing Claude's built-in behavior."
                />
                <textarea
                  id="settings-append-system-prompt"
                  value={settings.appendSystemPrompt}
                  onChange={(e) => updateSettings({ appendSystemPrompt: e.target.value })}
                  placeholder="e.g. Always respond in bullet points. Prefer TypeScript. Be concise."
                  rows={5}
                  maxLength={4000}
                  className="w-full resize-none rounded-lg border border-[rgba(255,135,175,0.15)] bg-[rgba(10,18,35,0.65)] px-3 py-2.5 text-[13px] leading-relaxed text-[var(--axon-text-secondary)] outline-none placeholder:text-[var(--axon-text-subtle)] focus:border-[rgba(255,135,175,0.3)] focus:bg-[rgba(10,18,35,0.82)] transition-all duration-200"
                  style={{ backdropFilter: 'blur(4px)' }}
                />
                <div className="mt-1.5 flex justify-between text-[10px] text-[var(--axon-text-subtle)]">
                  <span>Applied to every Pulse chat request</span>
                  <span>{settings.appendSystemPrompt.length} / 4000</span>
                </div>
              </section>

              <SectionDivider />

              {/* ── Tools & Permissions ──────────────────────────────── */}
              <section id="settings-section-tools" className="scroll-mt-20">
                <SectionHeader
                  icon={Wrench}
                  label="Tools & Permissions"
                  description="Fine-grained control over which tools Claude can use. Supports permission rule syntax (e.g. Bash(git log *), Read, Edit)."
                />
                <div className="space-y-5">
                  <div>
                    <label
                      htmlFor="settings-allowed-tools"
                      className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--axon-text-dim)]"
                    >
                      Allowed tools
                      <code className="ml-1.5 normal-case tracking-normal text-[var(--axon-text-subtle)]">
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
                      Comma-separated tools that execute without prompting for permission. Leave
                      blank for defaults.
                    </FieldHint>
                  </div>
                  <div>
                    <label
                      htmlFor="settings-disallowed-tools"
                      className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--axon-text-dim)]"
                    >
                      Disallowed tools
                      <code className="ml-1.5 normal-case tracking-normal text-[var(--axon-text-subtle)]">
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
                      Comma-separated tools removed from Claude's context entirely. Takes priority
                      over allowed tools.
                    </FieldHint>
                  </div>
                  <div>
                    <label
                      htmlFor="settings-tools-restrict"
                      className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--axon-text-dim)]"
                    >
                      Restrict built-in tools
                      <code className="ml-1.5 normal-case tracking-normal text-[var(--axon-text-subtle)]">
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
                      Restrict which built-in tools are available. Use empty string for all tools.
                      Different from --allowedTools.
                    </FieldHint>
                  </div>

                  <div
                    className="flex items-start gap-2.5 rounded-lg border border-[rgba(175,215,255,0.12)] px-3.5 py-3 transition-all duration-200"
                    style={{ background: 'rgba(10,18,35,0.58)', backdropFilter: 'blur(8px)' }}
                  >
                    <Info className="mt-0.5 size-3.5 shrink-0 text-[var(--axon-accent-pink)]" />
                    <p className="text-[11px] leading-relaxed text-[var(--axon-text-dim)]">
                      Pulse always runs with{' '}
                      <code className="rounded bg-[rgba(175,215,255,0.07)] px-1 py-0.5 font-mono text-[10px] text-[var(--axon-text-muted)]">
                        --dangerously-skip-permissions
                      </code>{' '}
                      because there is no TTY in the container environment. These tool filters layer
                      on top and do not restore the interactive permission prompt.
                    </p>
                  </div>
                </div>
              </section>

              <SectionDivider />

              {/* ── Session Behavior ─────────────────────────────────── */}
              <section id="settings-section-session" className="scroll-mt-20">
                <SectionHeader
                  icon={Terminal}
                  label="Session Behavior"
                  description="Control how the Claude CLI manages sessions and handles built-in commands during each chat."
                />
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
                      className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--axon-text-dim)]"
                    >
                      Fallback model
                      <code className="ml-1.5 normal-case tracking-normal text-[var(--axon-text-subtle)]">
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
                      className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--axon-text-dim)]"
                    >
                      Additional directories
                      <code className="ml-1.5 normal-case tracking-normal text-[var(--axon-text-subtle)]">
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
                      className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--axon-text-dim)]"
                    >
                      Beta features
                      <code className="ml-1.5 normal-case tracking-normal text-[var(--axon-text-subtle)]">
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

              {/* ── Keyboard Shortcuts ───────────────────────────────── */}
              <section id="settings-section-shortcuts" className="scroll-mt-20">
                <SectionHeader
                  icon={Zap}
                  label="Keyboard Shortcuts"
                  description="Global shortcuts available throughout the Pulse workspace and omnibox."
                />
                <div
                  className="overflow-hidden rounded-xl border border-[rgba(255,135,175,0.1)]"
                  style={{ background: 'rgba(10,18,35,0.58)', backdropFilter: 'blur(8px)' }}
                >
                  {KEYBOARD_SHORTCUTS.map(({ keys, desc }, idx) => (
                    <div
                      key={desc}
                      className={`flex items-center justify-between px-4 py-3 ${
                        idx < KEYBOARD_SHORTCUTS.length - 1
                          ? 'border-b border-[rgba(255,135,175,0.07)]'
                          : ''
                      }`}
                    >
                      <span className="text-[12px] text-[var(--axon-text-dim)]">{desc}</span>
                      <div className="flex items-center gap-1">
                        {keys.map((k, ki) => (
                          <span key={k} className="flex items-center gap-1">
                            {ki > 0 && (
                              <span className="text-[10px] text-[var(--axon-text-dim)]">+</span>
                            )}
                            <kbd className="rounded border border-[rgba(255,135,175,0.16)] bg-[rgba(10,18,35,0.6)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--axon-text-subtle)]">
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
            </div>
          </main>
        </div>
      </div>
    </div>
  )
}
