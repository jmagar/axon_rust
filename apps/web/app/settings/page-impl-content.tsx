'use client'

import { ArrowLeft, RotateCcw, SlidersHorizontal } from 'lucide-react'
import dynamic from 'next/dynamic'
import { useRouter } from 'next/navigation'
import { useEffect, useRef, useState } from 'react'
import type { NeuralCanvasHandle } from '@/components/neural-canvas'
import { DEFAULT_PULSE_SETTINGS, usePulseSettings } from '@/hooks/use-pulse-settings'
import { useWsMessageActions, useWsWorkspaceState } from '@/hooks/use-ws-messages'
import { Bot, NAV_SECTIONS, Server } from './settings-data'
import { SettingsSections } from './settings-sections'

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), { ssr: false })

export default function SettingsPage() {
  const router = useRouter()
  const { pulseAgent, pulseModel, pulsePermissionLevel, acpConfigOptions } = useWsWorkspaceState()
  const { setPulseModel, setPulsePermissionLevel } = useWsMessageActions()
  const { settings, updateSettings } = usePulseSettings()
  const [activeSection, setActiveSection] = useState('model')
  const canvasRef = useRef<NeuralCanvasHandle>(null)
  const [resetConfirming, setResetConfirming] = useState(false)

  useEffect(() => {
    canvasRef.current?.setIntensity(0.06)
  }, [])

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
            borderColor: 'var(--border-subtle)',
            background: 'rgba(3,7,18,0.86)',
            backdropFilter: 'blur(20px) saturate(180%)',
            height: '3.25rem',
          }}
        >
          <button
            type="button"
            onClick={() => router.back()}
            className="flex min-h-[44px] items-center gap-1.5 rounded-md px-2 py-1 text-[12px] font-medium text-[var(--text-dim)] transition-all duration-200 hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)] sm:min-h-0"
            aria-label="Go back"
          >
            <ArrowLeft className="size-3.5" />
            Back
          </button>
          <div className="h-4 w-px bg-[var(--border-subtle)]" />
          <div className="flex items-center gap-2">
            <SlidersHorizontal className="size-3.5 text-[var(--axon-primary-strong)]" />
            <h1 className="text-[14px] font-semibold text-[var(--text-primary)]">Settings</h1>
          </div>
          <div className="flex-1" />
          <button
            type="button"
            onClick={() => setResetConfirming(true)}
            className="flex min-h-[44px] items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-medium text-[var(--text-dim)] transition-all duration-200 hover:bg-[var(--surface-float)] hover:text-[var(--axon-primary)] sm:min-h-0"
            title="Reset all settings to defaults"
          >
            <RotateCcw className="size-3" />
            <span className="hidden sm:inline">Reset to defaults</span>
            <span className="sm:hidden">Reset</span>
          </button>
        </header>

        {/* Body */}
        <div className="flex flex-1">
          {/* Sidebar nav — hidden below lg breakpoint */}
          <nav
            className="sticky hidden h-[calc(100vh-3.25rem)] w-56 shrink-0 overflow-y-auto border-r border-r-[var(--border-subtle)] py-6 pr-4 lg:flex lg:flex-col"
            style={{
              top: '3.25rem',
              background: 'rgba(3,7,18,0.70)',
              backdropFilter: 'blur(16px)',
            }}
          >
            <p className="mb-2 px-2.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--text-dim)]">
              Configuration
            </p>
            {NAV_SECTIONS.map(({ id, label, icon: Icon }) => (
              <button
                key={id}
                type="button"
                onClick={() => scrollTo(id)}
                className={`flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[12px] font-medium transition-all duration-150 ${
                  activeSection === id
                    ? 'text-[var(--axon-primary)]'
                    : 'text-[var(--text-muted)] hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)]'
                }`}
                style={activeSection === id ? { background: 'rgba(255,135,175,0.12)' } : undefined}
              >
                <Icon className="size-3.5 shrink-0" />
                {label}
              </button>
            ))}

            {/* Related links */}
            <div className="mt-2 border-t border-[var(--border-subtle)] pt-3">
              <p className="mb-1.5 px-2.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--text-dim)]">
                Related
              </p>
              <button
                type="button"
                onClick={() => router.push('/settings/mcp')}
                className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[12px] font-medium text-[var(--text-muted)] transition-all duration-150 hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)]"
              >
                <Server className="size-3.5 shrink-0" />
                MCP Servers
              </button>
              <button
                type="button"
                onClick={() => router.push('/agents')}
                className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[12px] font-medium text-[var(--text-muted)] transition-all duration-150 hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)]"
              >
                <Bot className="size-3.5 shrink-0" />
                Agents
              </button>
            </div>

            <div className="mt-auto pt-4">
              <button
                type="button"
                onClick={() => setResetConfirming(true)}
                className="flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-[11px] font-medium text-[var(--text-dim)] transition-all duration-200 hover:bg-[var(--surface-float)] hover:text-[var(--axon-primary)]"
              >
                <RotateCcw className="size-3 shrink-0" />
                Reset all to defaults
              </button>
            </div>
          </nav>

          {/* Main content column */}
          <main className="flex-1 overflow-y-auto">
            <div className="mx-auto max-w-[780px] px-4 py-8 sm:px-6">
              <SettingsSections
                pulseAgent={pulseAgent}
                pulseModel={pulseModel}
                acpConfigOptions={acpConfigOptions}
                setPulseModel={setPulseModel}
                pulsePermissionLevel={pulsePermissionLevel}
                setPulsePermissionLevel={setPulsePermissionLevel}
                settings={settings}
                updateSettings={updateSettings}
              />
            </div>
          </main>
        </div>
      </div>

      {/* Reset confirmation modal */}
      {resetConfirming && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-[rgba(3,7,18,0.75)] backdrop-blur-sm animate-fade-in">
          <div className="w-full max-w-sm rounded-xl border border-[var(--border-standard)] bg-[var(--surface-base)] p-5 shadow-[var(--shadow-xl)] animate-scale-in">
            <div className="mb-1 flex items-center gap-2">
              <RotateCcw className="size-4 text-[var(--axon-secondary)]" />
              <h3 className="font-display text-sm font-semibold text-[var(--text-primary)]">
                Reset all settings?
              </h3>
            </div>
            <p className="mb-4 text-xs text-[var(--text-muted)]">
              This will restore all settings to their defaults. Your MCP server configurations will
              not be affected.
            </p>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setResetConfirming(false)}
                className="rounded-md border border-[var(--border-subtle)] bg-transparent px-3 py-1.5 text-xs text-[var(--text-secondary)] hover:bg-[var(--surface-float)] transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => {
                  handleReset()
                  setResetConfirming(false)
                }}
                className="rounded-md bg-[rgba(255,135,175,0.15)] border border-[var(--border-accent)] px-3 py-1.5 text-xs text-[var(--axon-secondary)] hover:bg-[rgba(255,135,175,0.25)] transition-colors"
              >
                Reset
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
