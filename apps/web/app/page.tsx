'use client'

import dynamic from 'next/dynamic'
import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useRef, useState } from 'react'
import { DockerStats } from '@/components/docker-stats'
import { LandingCards } from '@/components/landing-cards'
import type { NeuralCanvasHandle } from '@/components/neural-canvas'
import { Omnibox } from '@/components/omnibox'

const PulseEditorPane = dynamic(
  () =>
    import('@/components/pulse/pulse-editor-pane').then((m) => ({ default: m.PulseEditorPane })),
  { ssr: false },
)

import { PulseMobilePaneSwitcher } from '@/components/pulse/pulse-mobile-pane-switcher'
import { ResultsPanel } from '@/components/results-panel'
import { WsIndicator } from '@/components/ws-indicator'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { MOBILE_PANE_STORAGE_KEY } from '@/hooks/use-split-pane'
import { useWsMessages } from '@/hooks/use-ws-messages'
import { setPendingTab } from '@/lib/pending-tab'
import {
  DEFAULT_NEURAL_CANVAS_PROFILE,
  type NeuralCanvasProfile,
} from '@/lib/pulse/neural-canvas-presets'
import { resultToMarkdown } from '@/lib/result-to-markdown'
import { getStorageItem, setStorageItem } from '@/lib/storage'
import type { ContainerStats, WsServerMsg } from '@/lib/ws-protocol'

const INLINE_RESULT_MODES = new Set(['ask', 'research', 'query', 'retrieve'])

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), {
  ssr: false,
})
const CANVAS_PROFILE_STORAGE_KEY = 'axon.web.neural-canvas.profile'
const CANVAS_PROFILE_OPTIONS: NeuralCanvasProfile[] = ['current', 'subtle', 'cinematic', 'electric']

export default function DashboardPage() {
  const router = useRouter()
  const { subscribe } = useAxonWs()
  const canvasRef = useRef<NeuralCanvasHandle>(null)
  const { isProcessing, hasResults, currentMode, workspaceMode, workspacePromptVersion } =
    useWsMessages()
  const [canvasProfile, setCanvasProfile] = useState<NeuralCanvasProfile>(
    DEFAULT_NEURAL_CANVAS_PROFILE,
  )
  const [landingMobilePane, setLandingMobilePane] = useState<'chat' | 'editor'>('chat')
  const [landingEditorMarkdown, setLandingEditorMarkdown] = useState('')

  // Persist landing editor content across tab switches / page unloads
  const LANDING_EDITOR_KEY = 'axon.web.landing.editor-content'
  useEffect(() => {
    const saved = getStorageItem(LANDING_EDITOR_KEY)
    if (saved) setLandingEditorMarkdown(saved)
  }, [])

  const handleLandingEditorChange = useCallback((md: string) => {
    setLandingEditorMarkdown(md)
    setStorageItem(LANDING_EDITOR_KEY, md)
  }, [])

  useEffect(() => {
    const raw = getStorageItem(CANVAS_PROFILE_STORAGE_KEY)
    if (raw && CANVAS_PROFILE_OPTIONS.includes(raw as NeuralCanvasProfile)) {
      setCanvasProfile(raw as NeuralCanvasProfile)
    }
  }, [])

  useEffect(() => {
    const saved = getStorageItem(MOBILE_PANE_STORAGE_KEY)
    if (saved === 'chat' || saved === 'editor') setLandingMobilePane(saved)
  }, [])

  const handleLandingMobilePaneChange = useCallback((pane: 'chat' | 'editor') => {
    setLandingMobilePane(pane)
    setStorageItem(MOBILE_PANE_STORAGE_KEY, pane)
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

  // For inline-result modes (ask, research, query, retrieve): auto-open results in editor tab
  const capturedJsonRef = useRef<unknown[]>([])
  const currentModeRef = useRef(currentMode)
  useEffect(() => {
    currentModeRef.current = currentMode
  }, [currentMode])
  useEffect(() => {
    if (isProcessing) capturedJsonRef.current = []
  }, [isProcessing])
  useEffect(() => {
    return subscribe((msg: WsServerMsg) => {
      if (msg.type === 'command.output.json') {
        if (INLINE_RESULT_MODES.has(currentModeRef.current)) {
          capturedJsonRef.current = [...capturedJsonRef.current, msg.data.data]
        }
      } else if (msg.type === 'command.done') {
        const mode = currentModeRef.current
        if (INLINE_RESULT_MODES.has(mode) && capturedJsonRef.current.length > 0) {
          const markdown = resultToMarkdown(mode, capturedJsonRef.current)
          if (markdown) {
            const label = mode.charAt(0).toUpperCase() + mode.slice(1)
            setPendingTab({ title: label, markdown })
            router.push('/editor')
            capturedJsonRef.current = []
          }
        }
      }
    })
  }, [subscribe, router])

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

  const isInlineMode = INLINE_RESULT_MODES.has(currentMode)
  const isPulseWorkspaceActive =
    workspaceMode === 'pulse' && hasResults && workspacePromptVersion > 0

  return (
    <>
      <NeuralCanvas ref={canvasRef} profile={canvasProfile} />
      <WsIndicator />
      {isPulseWorkspaceActive ? (
        /* Full-screen workspace — fixed overlay from sidebar right-edge to viewport edge */
        <div
          className="fixed bottom-0 right-0 top-0 z-[3] overflow-hidden"
          style={{ left: 'var(--sidebar-w, 260px)' }}
        >
          <ResultsPanel statsSlot={<DockerStats onStats={handleStats} />} />
        </div>
      ) : (
        /* Landing / results — centered glass card */
        <main
          className={`relative z-[1] mx-auto max-w-[1180px] transition-[padding] duration-500 ease-[cubic-bezier(0.4,0,0.2,1)] xl:max-w-[1240px] ${
            hasResults
              ? 'px-2.5 pb-5 pt-12 sm:px-3.5 sm:pb-8'
              : landingMobilePane === 'editor'
                ? 'px-2.5 pb-5 pt-11 sm:px-3.5 sm:pb-8 sm:pt-[40vh]'
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
              <div
                className={`order-1 scale-100 ${landingMobilePane === 'editor' ? 'hidden lg:block' : 'block'}`}
              >
                <Omnibox />
                {!hasResults && <LandingCards />}
              </div>
              <div className="order-2">
                {landingMobilePane === 'editor' && !hasResults && (
                  <div className="flex h-[calc(100dvh-5rem)] overflow-hidden rounded-xl border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.5)] lg:hidden">
                    <PulseEditorPane
                      markdown={landingEditorMarkdown}
                      onMarkdownChange={handleLandingEditorChange}
                      scrollStorageKey="axon.web.landing.editor-scroll"
                    />
                  </div>
                )}
                {!isInlineMode && (
                  <div
                    className={
                      landingMobilePane === 'editor' && !hasResults ? 'hidden lg:block' : undefined
                    }
                  >
                    <ResultsPanel statsSlot={<DockerStats onStats={handleStats} />} />
                  </div>
                )}
              </div>
            </div>
          </div>
        </main>
      )}

      {/* Fixed top-right — pane switcher (landing + mobile) */}
      <div className="fixed right-28 top-0 z-10 flex h-11 items-center gap-2">
        {!hasResults && (
          <div className="lg:hidden">
            <PulseMobilePaneSwitcher
              mobilePane={landingMobilePane}
              onMobilePaneChange={handleLandingMobilePaneChange}
            />
          </div>
        )}
      </div>

      {/* Fixed bottom omnibox — only when Pulse workspace is active */}
      {isPulseWorkspaceActive && (
        <div
          className="fixed bottom-0 right-0 z-20 px-2.5 pb-3 sm:px-3.5 sm:pb-4"
          style={{ left: 'var(--sidebar-w, 260px)' }}
        >
          <div className="mx-auto max-w-[1180px] xl:max-w-[1240px]">
            <div
              className="rounded-xl border p-1 backdrop-blur-xl"
              style={{
                borderColor: isProcessing ? 'rgba(175,215,255,0.25)' : 'var(--border-subtle)',
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
