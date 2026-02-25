'use client'

import { Settings2 } from 'lucide-react'
import dynamic from 'next/dynamic'
import { useCallback, useEffect, useRef, useState } from 'react'
import { DockerStats } from '@/components/docker-stats'
import type { NeuralCanvasHandle } from '@/components/neural-canvas'
import { Omnibox } from '@/components/omnibox'
import { ResultsPanel } from '@/components/results-panel'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { WsIndicator } from '@/components/ws-indicator'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { useWsMessages } from '@/hooks/use-ws-messages'
import {
  DEFAULT_NEURAL_CANVAS_PROFILE,
  type NeuralCanvasProfile,
} from '@/lib/pulse/neural-canvas-presets'
import type { ContainerStats, WsServerMsg } from '@/lib/ws-protocol'

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), { ssr: false })
const CANVAS_PROFILE_STORAGE_KEY = 'axon.web.neural-canvas.profile'
const CANVAS_PROFILE_OPTIONS: NeuralCanvasProfile[] = ['current', 'subtle', 'cinematic', 'electric']

export default function DashboardPage() {
  const { subscribe } = useAxonWs()
  const canvasRef = useRef<NeuralCanvasHandle>(null)
  const { isProcessing, hasResults } = useWsMessages()
  const [canvasProfile, setCanvasProfile] = useState<NeuralCanvasProfile>(
    DEFAULT_NEURAL_CANVAS_PROFILE,
  )

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

  const handleCanvasProfileChange = useCallback((value: string) => {
    if (!CANVAS_PROFILE_OPTIONS.includes(value as NeuralCanvasProfile)) return
    const profile = value as NeuralCanvasProfile
    setCanvasProfile(profile)
    try {
      window.localStorage.setItem(CANVAS_PROFILE_STORAGE_KEY, profile)
    } catch {
      // Ignore storage errors.
    }
  }, [])

  // Canvas intensity: full on execute start, pulse on done/error
  useEffect(() => {
    return subscribe((msg: WsServerMsg) => {
      if (msg.type === 'done' || msg.type === 'error') {
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

  return (
    <>
      <NeuralCanvas ref={canvasRef} profile={canvasProfile} />
      <WsIndicator />
      <div className="fixed right-5 top-4 z-20">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              aria-label="Canvas settings"
              className="inline-flex size-9 items-center justify-center rounded-full border border-[rgba(175,215,255,0.28)] bg-[rgba(8,16,34,0.55)] text-[rgba(220,238,255,0.92)] backdrop-blur-md transition-colors hover:bg-[rgba(12,26,52,0.74)]"
            >
              <Settings2 className="size-4" />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="min-w-44">
            <DropdownMenuLabel>Canvas Preset</DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuRadioGroup value={canvasProfile} onValueChange={handleCanvasProfileChange}>
              <DropdownMenuRadioItem value="current">Current</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="subtle">Subtle</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="cinematic">Cinematic</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="electric">Electric</DropdownMenuRadioItem>
            </DropdownMenuRadioGroup>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      {/* Gradient logo — fixed top-left */}
      <div className="fixed left-6 top-5 z-10 select-none">
        <h1
          className="text-base font-extrabold tracking-[6px]"
          style={{
            background: 'linear-gradient(135deg, #ff87af 0%, #afd7ff 50%, #8787af 100%)',
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
        className={`relative z-[1] mx-auto max-w-[1060px] transition-[padding] duration-500 ease-[cubic-bezier(0.4,0,0.2,1)] ${
          hasResults
            ? 'px-3 pb-6 pt-14 sm:px-5 sm:pb-10'
            : 'px-3 pb-6 pt-[35vh] sm:px-5 sm:pb-10 sm:pt-[40vh]'
        }`}
      >
        {/* Interface card — glass-morphic */}
        <div
          className={`rounded-2xl border p-3 transition-all duration-500 sm:p-5 ${
            isProcessing
              ? 'border-[rgba(255,135,175,0.3)] shadow-[0_0_80px_rgba(255,135,175,0.1),0_0_30px_rgba(175,215,255,0.05),inset_0_1px_0_rgba(255,255,255,0.04)]'
              : 'border-[rgba(175,215,255,0.12)] shadow-[0_0_60px_rgba(175,215,255,0.05),inset_0_1px_0_rgba(255,255,255,0.02)]'
          }`}
          style={{ background: 'rgba(15, 23, 42, 0.08)' }}
        >
          <Omnibox />
          <ResultsPanel statsSlot={<DockerStats onStats={handleStats} />} />
        </div>
      </main>
    </>
  )
}
