export const JOB_TYPES = ['crawl', 'extract', 'embed', 'ingest'] as const
export type JobType = (typeof JOB_TYPES)[number]

export const JOB_STATUSES = ['pending', 'running', 'completed', 'failed', 'canceled'] as const
export type JobStatus = (typeof JOB_STATUSES)[number]

const JOB_STATUS_SET = new Set<string>(JOB_STATUSES)

export function safeStatus(status: string): JobStatus {
  return JOB_STATUS_SET.has(status) ? (status as JobStatus) : 'pending'
}
