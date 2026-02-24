'use client'

import dynamic from 'next/dynamic'
import { useCallback, useEffect, useRef, useState } from 'react'
import { DockerStats } from '@/components/docker-stats'
import type { NeuralCanvasHandle } from '@/components/neural-canvas'
import { Omnibox, type OmniboxHandle } from '@/components/omnibox'
import { type OutputLine, type RecentRun, ResultsPanel } from '@/components/results-panel'
import { WsIndicator } from '@/components/ws-indicator'
import { useAxonWs } from '@/hooks/use-axon-ws'
import type { ContainerStats, WsServerMsg } from '@/lib/ws-protocol'

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), { ssr: false })

export default function DashboardPage() {
  const { subscribe } = useAxonWs()
  const omniboxRef = useRef<OmniboxHandle>(null)
  const canvasRef = useRef<NeuralCanvasHandle>(null)
  const [lines, setLines] = useState<OutputLine[]>([])
  const [recentRuns, setRecentRuns] = useState<RecentRun[]>([])
  const [isProcessing, setIsProcessing] = useState(false)
  const currentModeRef = useRef('')

  // Subscribe to WS messages and dispatch
  useEffect(() => {
    return subscribe((msg: WsServerMsg) => {
      switch (msg.type) {
        case 'output': {
          let parsed: Record<string, unknown> | undefined
          try {
            parsed = JSON.parse(msg.line)
          } catch {
            /* plain text */
          }
          setLines((prev) => [...prev, { type: 'output', content: msg.line, parsed }])
          break
        }
        case 'log':
          setLines((prev) => [...prev, { type: 'log', content: msg.line }])
          break
        case 'done':
          omniboxRef.current?.handleDone(msg.elapsed_ms, msg.exit_code)
          setIsProcessing(false)
          // Add to recent runs
          setRecentRuns((prev) => {
            const run: RecentRun = {
              status: 'done',
              mode: currentModeRef.current,
              target: '',
              duration: `${(msg.elapsed_ms / 1000).toFixed(1)}s`,
              lines: 0,
              time: new Date().toLocaleTimeString(),
            }
            return [run, ...prev].slice(0, 20)
          })
          // Decay neural intensity
          canvasRef.current?.setIntensity(0.15)
          setTimeout(() => canvasRef.current?.setIntensity(0), 3000)
          break
        case 'error':
          omniboxRef.current?.handleError(msg.message, msg.elapsed_ms)
          setIsProcessing(false)
          setLines((prev) => [...prev, { type: 'error', content: msg.message }])
          setRecentRuns((prev) => {
            const run: RecentRun = {
              status: 'failed',
              mode: currentModeRef.current,
              target: '',
              duration: msg.elapsed_ms ? `${(msg.elapsed_ms / 1000).toFixed(1)}s` : '0s',
              lines: 0,
              time: new Date().toLocaleTimeString(),
            }
            return [run, ...prev].slice(0, 20)
          })
          canvasRef.current?.setIntensity(0.15)
          setTimeout(() => canvasRef.current?.setIntensity(0), 3000)
          break
        // stats handled by DockerStats component directly
      }
    })
  }, [subscribe])

  const handleExecute = useCallback((mode: string, _input: string) => {
    currentModeRef.current = mode
    setLines([])
    setIsProcessing(true)
    // Fire neural intensity on command execution
    canvasRef.current?.setIntensity(1)
  }, [])

  const handleStats = useCallback(
    (data: {
      aggregate: { cpu_percent: number }
      containers: Record<string, ContainerStats>
      container_count: number
    }) => {
      canvasRef.current?.stimulate(data.containers)
      // Map aggregate CPU to background neural intensity when not processing
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

      {/* Logo */}
      <div className="fixed left-6 top-6 z-10 select-none">
        <h1 className="text-xl font-bold tracking-tight text-foreground/90">
          axon<span className="text-primary">.</span>
        </h1>
      </div>

      {/* Main content */}
      <main className="relative z-[1] mx-auto flex min-h-screen max-w-4xl flex-col items-center justify-center px-6 py-16">
        <div className="w-full space-y-6 rounded-2xl border border-border/30 bg-card/40 p-6 backdrop-blur-xl">
          <Omnibox ref={omniboxRef} onExecute={handleExecute} />
          <ResultsPanel
            lines={lines}
            recentRuns={recentRuns}
            isProcessing={isProcessing}
            statsSlot={<DockerStats onStats={handleStats} />}
          />
        </div>
      </main>
    </>
  )
}
