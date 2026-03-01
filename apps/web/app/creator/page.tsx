import type { Metadata } from 'next'
import { CreatorDashboard } from '@/components/creator/creator-dashboard'

export const metadata: Metadata = {
  title: 'Creator — Axon',
}

export default function CreatorPage() {
  return <CreatorDashboard />
}
