import type { Metadata } from 'next'
import { TasksDashboard } from '@/components/tasks/tasks-dashboard'

export const metadata: Metadata = { title: 'Tasks — Axon' }

export default function TasksPage() {
  return <TasksDashboard />
}
