'use client'

import { Bot, Network, Settings2 } from 'lucide-react'
import dynamic from 'next/dynamic'
import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useRef, useState } from 'react'
import { DockerStats } from '@/components/docker-stats'
import { LandingCards } from '@/components/landing-cards'
import type { NeuralCanvasHandle } from '@/components/neural-canvas'
import { Omnibox } from '@/components/omnibox'
import { PulseMobilePaneSwitcher } from '@/components/pulse/pulse-mobile-pane-switcher'
import { ResultsPanel } from '@/components/results-panel'
import { WsIndicator } from '@/components/ws-indicator'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { MOBILE_PANE_STORAGE_KEY } from '@/hooks/use-split-pane'
import { useWsMessages } from '@/hooks/use-ws-messages'
import {
  DEFAULT_NEURAL_CANVAS_PROFILE,
  type NeuralCanvasProfile,
} from '@/lib/pulse/neural-canvas-presets'
import type { ContainerStats, WsServerMsg } from '@/lib/ws-protocol'

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), {
  ssr: false,
})
const CANVAS_PROFILE_STORAGE_KEY = 'axon.web.neural-canvas.profile'
const CANVAS_PROFILE_OPTIONS: NeuralCanvasProfile[] = ['current', 'subtle', 'cinematic', 'electric']

export default function DashboardPage() {
  const router = useRouter()
  const { subscribe } = useAxonWs()
  const canvasRef = useRef<NeuralCanvasHandle>(null)
  const { isProcessing, hasResults, workspaceMode, workspacePromptVersion } = useWsMessages()
  const [canvasProfile, setCanvasProfile] = useState<NeuralCanvasProfile>(
    DEFAULT_NEURAL_CANVAS_PROFILE,
  )
  const [landingMobilePane, setLandingMobilePane] = useState<'chat' | 'editor'>('chat')

  useEffect(() => {
    try {
      const raw = window.localStorage.getItem(CANVAS_PROFILE_STORAGE_KEY)
      if (!raw) return
      if (CANVAS_PROFILE_OPTIONS.includes(raw as NeuralCanvasProfile)) {
        setCanvasProfile(raw as NeuralCanvasProfile)
      }
    } catch {
      // Ignore storage errors and keep default profile.
    }
  }, [])

  useEffect(() => {
    try {
      const saved = window.localStorage.getItem(MOBILE_PANE_STORAGE_KEY)
      if (saved === 'chat' || saved === 'editor') setLandingMobilePane(saved)
    } catch {
      // Ignore storage errors.
    }
  }, [])

  const handleLandingMobilePaneChange = useCallback((pane: 'chat' | 'editor') => {
    setLandingMobilePane(pane)
    try {
      window.localStorage.setItem(MOBILE_PANE_STORAGE_KEY, pane)
    } catch {
      // Ignore storage errors.
    }
  }, [])

  // Canvas intensity: full on execute start, pulse on command done/error.
  useEffect(() => {
    return subscribe((msg: WsServerMsg) => {
      if (msg.type === 'command.done' || msg.type === 'command.error') {
        canvasRef.current?.setIntensity(0.15)
        setTimeout(() => canvasRef.current?.setIntensity(0), 3000)
      }
    })
  }, [subscribe])

  // Drive canvas to full intensity when processing starts
  useEffect(() => {
    if (isProcessing) {
      canvasRef.current?.setIntensity(1)
    }
  }, [isProcessing])

  const handleStats = useCallback(
    (data: {
      aggregate: { cpu_percent: number }
      containers: Record<string, ContainerStats>
      container_count: number
    }) => {
      canvasRef.current?.stimulate(data.containers)
      if (!isProcessing) {
        const maxCpu = data.container_count * 100
        const norm = Math.min(data.aggregate.cpu_percent / maxCpu, 1.0)
        canvasRef.current?.setIntensity(0.02 + norm * 0.83)
      }
    },
    [isProcessing],
  )

  const isPulseWorkspaceActive =
    workspaceMode === 'pulse' && hasResults && workspacePromptVersion > 0

  return (
    <>
      <NeuralCanvas ref={canvasRef} profile={canvasProfile} />
      <WsIndicator />
      {/* Gradient logo — fixed top-left */}
      <div className="fixed left-6 top-5 z-10 select-none">
        <h1
          className="text-base font-extrabold tracking-[6px]"
          style={{
            background: 'linear-gradient(135deg, #afd7ff 0%, #ff87af 50%, #8787af 100%)',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
            backgroundClip: 'text',
          }}
        >
          AXON
        </h1>
      </div>

      {/* Main container — centered vertically, slides up on results */}
      <main
        className={`relative z-[1] mx-auto max-w-[1180px] transition-[padding] duration-500 ease-[cubic-bezier(0.4,0,0.2,1)] xl:max-w-[1240px] ${
          hasResults
            ? `px-2.5 sm:px-3.5 ${isPulseWorkspaceActive ? 'pt-1 pb-[80px] lg:pt-12 sm:pb-[88px]' : 'pt-12 pb-5 sm:pb-8'}`
            : 'px-2.5 pb-5 pt-[35vh] sm:px-3.5 sm:pb-8 sm:pt-[40vh]'
        }`}
      >
        {/* Interface card — glass-morphic */}
        <div
          className={`rounded-2xl border p-2 transition-all duration-500 sm:p-3 ${
            isProcessing
              ? 'shadow-[0_0_80px_rgba(175,215,255,0.1),0_0_30px_rgba(255,135,175,0.05),inset_0_1px_0_rgba(255,255,255,0.04)]'
              : 'shadow-[0_0_60px_rgba(255,135,175,0.05),inset_0_1px_0_rgba(255,255,255,0.02)]'
          }`}
          style={{
            borderColor: isProcessing ? 'rgba(175,215,255,0.3)' : 'var(--axon-border)',
            background: 'var(--axon-surface-3)',
          }}
        >
          <div className="flex flex-col gap-2">
            {!isPulseWorkspaceActive && (
              <div
                className={`order-1 scale-100 ${landingMobilePane === 'editor' ? 'hidden lg:block' : 'block'}`}
              >
                <Omnibox />
                {!hasResults && <LandingCards />}
              </div>
            )}
            <div className={isPulseWorkspaceActive ? 'order-1' : 'order-2'}>
              {!isPulseWorkspaceActive && landingMobilePane === 'editor' && !hasResults && (
                <div className="flex items-center justify-center rounded-xl border border-[rgba(255,135,175,0.1)] py-14 text-sm text-[var(--axon-text-dim)] lg:hidden">
                  Run a command to see results here
                </div>
              )}
              <ResultsPanel statsSlot={<DockerStats onStats={handleStats} />} />
            </div>
          </div>
        </div>
      </main>

      {/* Fixed top-right — pane switcher (landing + mobile) + nav icons (always) */}
      <div className="fixed right-3 top-0 z-10 flex h-11 items-center gap-2">
        {!hasResults && (
          <div className="lg:hidden">
            <PulseMobilePaneSwitcher
              mobilePane={landingMobilePane}
              onMobilePaneChange={handleLandingMobilePaneChange}
            />
          </div>
        )}
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => router.push('/mcp')}
            title="MCP Servers"
            aria-label="MCP Servers"
            className="flex items-center justify-center size-7 rounded border border-[rgba(255,135,175,0.12)] bg-[rgba(10,18,35,0.42)] text-[var(--axon-text-dim)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-accent-pink)] backdrop-blur-sm"
          >
            <Network className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={() => router.push('/agents')}
            title="Available Agents"
            aria-label="Available Agents"
            className="flex items-center justify-center size-7 rounded border border-[rgba(255,135,175,0.12)] bg-[rgba(10,18,35,0.42)] text-[var(--axon-text-dim)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-accent-pink)] backdrop-blur-sm"
          >
            <Bot className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={() => router.push('/settings')}
            title="Settings"
            aria-label="Open settings"
            className="flex items-center justify-center size-7 rounded border border-[rgba(255,135,175,0.12)] bg-[rgba(10,18,35,0.42)] text-[var(--axon-text-dim)] transition-colors hover:border-[rgba(175,215,255,0.25)] hover:text-[var(--axon-accent-pink)] backdrop-blur-sm"
          >
            <Settings2 className="size-3.5" />
          </button>
        </div>
      </div>

      {/* Fixed bottom omnibox — only when Pulse workspace is active */}
      {isPulseWorkspaceActive && (
        <div className="fixed bottom-0 left-0 right-0 z-20 px-2.5 pb-3 sm:px-3.5 sm:pb-4">
          <div className="mx-auto max-w-[1180px] xl:max-w-[1240px]">
            <div
              className="rounded-xl border p-1 backdrop-blur-xl"
              style={{
                borderColor: isProcessing ? 'rgba(175,215,255,0.25)' : 'rgba(255,135,175,0.12)',
                background: 'rgba(10,18,35,0.85)',
              }}
            >
              <Omnibox />
            </div>
          </div>
        </div>
      )}
    </>
  )
}
