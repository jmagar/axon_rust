import type { Metadata } from 'next'
import { SourcesDashboard } from '@/components/cortex/sources-dashboard'

export const metadata: Metadata = { title: 'Sources — Axon' }

export default function SourcesPage() {
  return <SourcesDashboard />
}
