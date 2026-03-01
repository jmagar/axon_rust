'use client'

import dynamic from 'next/dynamic'
import type { ReactNode } from 'react'
import { useWsMessages } from '@/hooks/use-ws-messages'
import { PulseSidebar } from './pulse/sidebar/pulse-sidebar'

const NeuralCanvas = dynamic(() => import('@/components/neural-canvas'), { ssr: false })

export function AppShell({ children }: { children: ReactNode }) {
  const { crawlFiles, selectedFile, selectFile, currentJobId } = useWsMessages()

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <NeuralCanvas profile="subtle" />
      <PulseSidebar
        crawlFiles={crawlFiles}
        selectedFile={selectedFile}
        onSelectFile={selectFile}
        jobId={currentJobId}
      />
      <div className="relative z-[1] min-w-0 flex-1 overflow-y-auto">{children}</div>
    </div>
  )
}
