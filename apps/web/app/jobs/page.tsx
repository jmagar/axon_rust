import type { Metadata } from 'next'
import { JobsDashboard } from '@/components/jobs/jobs-dashboard'

export const metadata: Metadata = {
  title: 'Jobs — Axon',
  description: 'RAG pipeline jobs dashboard — crawl, extract, embed, and ingest jobs.',
}

export default function JobsPage() {
  return <JobsDashboard />
}
