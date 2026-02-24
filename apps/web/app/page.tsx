'use client'

import dynamic from 'next/dynamic'
import { useCallback, useEffect, useRef } from 'react'
import { DockerStats } from '@/components/docker-stats'
import type { NeuralCanvasHandle } from '@/components/neural-canvas'
import { Omnibox } from '@/components/omnibox'
import { ResultsPanel } from '@/components/results-panel'
import { WsIndicator } from '@/components/ws-indicator'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { useWsMessages } from '@/hooks/use-ws-messages'
import type { ContainerStats, WsServerMsg } from '@/lib/ws-protocol'

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), { ssr: false })

export default function DashboardPage() {
  const { subscribe } = useAxonWs()
  const canvasRef = useRef<NeuralCanvasHandle>(null)
  const { isProcessing, hasResults } = useWsMessages()

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
      <NeuralCanvas ref={canvasRef} />
      <WsIndicator />

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
