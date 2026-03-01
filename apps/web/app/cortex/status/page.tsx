import type { Metadata } from 'next'
import { StatusDashboard } from '@/components/cortex/status-dashboard'

export const metadata: Metadata = { title: 'Status — Axon' }

export default function StatusPage() {
  return <StatusDashboard />
}
