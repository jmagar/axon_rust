import type { Metadata } from 'next'
import { DoctorDashboard } from '@/components/cortex/doctor-dashboard'

export const metadata: Metadata = { title: 'Doctor — Axon' }

export default function DoctorPage() {
  return <DoctorDashboard />
}
