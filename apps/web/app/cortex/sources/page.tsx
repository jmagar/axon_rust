import type { Metadata } from 'next'
import { Suspense } from 'react'
import { SourcesDashboard } from '@/components/cortex/sources-dashboard'

export const metadata: Metadata = { title: 'Sources — Axon' }

export default function SourcesPage() {
  return (
    <Suspense>
      <SourcesDashboard />
    </Suspense>
  )
}
