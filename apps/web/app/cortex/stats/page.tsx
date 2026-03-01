import type { Metadata } from 'next'
import { StatsDashboard } from '@/components/cortex/stats-dashboard'

export const metadata: Metadata = { title: 'Stats — Axon' }

export default function StatsPage() {
  return <StatsDashboard />
}
