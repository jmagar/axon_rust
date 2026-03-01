import type { Metadata } from 'next'
import { DomainsDashboard } from '@/components/cortex/domains-dashboard'

export const metadata: Metadata = { title: 'Domains — Axon' }

export default function DomainsPage() {
  return <DomainsDashboard />
}
