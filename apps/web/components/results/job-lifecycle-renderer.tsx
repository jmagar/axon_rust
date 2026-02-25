'use client'

import { useCallback, useEffect, useMemo, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { summarizeStructuredValue } from '@/lib/structured-text'
import type { WsServerMsg } from '@/lib/ws-protocol'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type JobPhase = 'enqueued' | 'running' | 'completed' | 'failed' | 'canceled'

interface JobState {
  jobId: string
  status: JobPhase
  errorText?: string
  resultSummary?: Record<string, unknown>
}

interface JobLifecycleRendererProps {
  stdoutJson: unknown[]
  commandMode: string | null
  isProcessing: boolean
  errorMessage: string
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

const PHASE_META: Record<JobPhase, { color: string; label: string; dotClass: string }> = {
  enqueued: {
    color: '#ffaf87',
    label: 'Enqueued',
    dotClass: 'bg-[#ffaf87] shadow-[0_0_6px_rgba(255,175,135,0.5)]',
  },
  running: {
    color: '#87afff',
    label: 'Running',
    dotClass: 'animate-pulse bg-[#87afff] shadow-[0_0_8px_rgba(135,175,255,0.6)]',
  },
  completed: {
    color: '#87d787',
    label: 'Completed',
    dotClass: 'bg-[#87d787] shadow-[0_0_6px_rgba(135,215,135,0.5)]',
  },
  failed: {
    color: '#ff5f87',
    label: 'Failed',
    dotClass: 'bg-[#ff5f87] shadow-[0_0_6px_rgba(255,95,135,0.5)]',
  },
  canceled: {
    color: '#8787af',
    label: 'Canceled',
    dotClass: 'bg-[#8787af] shadow-[0_0_6px_rgba(135,135,175,0.4)]',
  },
}

function normalizePhase(raw: string): JobPhase {
  const lower = raw.toLowerCase()
  if (lower === 'completed' || lower === 'done') return 'completed'
  if (lower === 'failed' || lower === 'error') return 'failed'
  if (lower === 'canceled' || lower === 'cancelled') return 'canceled'
  if (lower === 'running' || lower === 'processing') return 'running'
  return 'enqueued'
}

/** Extract job state from a stdout JSON object. */
function extractJobState(obj: unknown): JobState | null {
  if (!isRecord(obj)) return null

  const jobId = (obj.job_id ?? obj.id ?? '') as string
  if (!jobId) return null

  const rawStatus = (obj.status ?? 'enqueued') as string
  const errorText = (obj.error_text ?? obj.error ?? undefined) as string | undefined

  // Collect all fields that aren't job metadata as result summary
  const metaKeys = new Set(['job_id', 'id', 'status', 'error_text', 'error', 'type'])
  const summary: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(obj)) {
    if (!metaKeys.has(k) && v !== null && v !== undefined) {
      summary[k] = v
    }
  }

  return {
    jobId,
    status: normalizePhase(rawStatus),
    errorText,
    resultSummary: Object.keys(summary).length > 0 ? summary : undefined,
  }
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function JobCard({ job, commandMode }: { job: JobState; commandMode: string | null }) {
  const { send } = useAxonWs()
  const [cancelSent, setCancelSent] = useState(false)

  const handleCancel = useCallback(() => {
    if (!commandMode || cancelSent) return
    send({
      type: 'execute',
      mode: commandMode,
      input: `cancel ${job.jobId}`,
      flags: {},
    })
    setCancelSent(true)
  }, [send, commandMode, job.jobId, cancelSent])

  const handleCheckStatus = useCallback(() => {
    if (!commandMode) return
    send({
      type: 'execute',
      mode: commandMode,
      input: `status ${job.jobId}`,
      flags: {},
    })
  }, [send, commandMode, job.jobId])

  const handleViewErrors = useCallback(() => {
    if (!commandMode) return
    send({
      type: 'execute',
      mode: commandMode,
      input: `errors ${job.jobId}`,
      flags: {},
    })
  }, [send, commandMode, job.jobId])

  const isTerminal =
    job.status === 'completed' || job.status === 'failed' || job.status === 'canceled'
  const canCancel = !isTerminal && !cancelSent

  return (
    <div
      className="rounded-lg border border-[rgba(175,215,255,0.08)] p-4"
      style={{ background: 'rgba(10, 18, 35, 0.3)' }}
    >
      {/* Header: status dot + job ID */}
      <div className="mb-3 flex items-center gap-3">
        <span
          className={`inline-block size-2 shrink-0 rounded-full ${PHASE_META[job.status].dotClass}`}
        />
        <div className="min-w-0 flex-1">
          <span className="font-mono text-[12px] text-[#afd7ff]">{job.jobId}</span>
          <span
            className="ml-2 text-[11px] font-semibold uppercase tracking-wider"
            style={{ color: PHASE_META[job.status].color }}
          >
            {PHASE_META[job.status].label}
          </span>
        </div>
      </div>

      {/* Error display */}
      {job.status === 'failed' && job.errorText && (
        <div className="mb-3 rounded-md border border-[rgba(255,95,135,0.2)] bg-[rgba(255,95,135,0.06)] px-3 py-2">
          <div className="mb-1 text-[10px] font-bold uppercase tracking-wider text-[#ff5f87]">
            Error
          </div>
          <div className="font-mono text-[12px] leading-relaxed text-[#dce6f0]">
            {job.errorText}
          </div>
        </div>
      )}

      {/* Result summary for completed jobs */}
      {job.status === 'completed' && job.resultSummary && (
        <div className="mb-3 space-y-0.5">
          <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-[#5f87af]">
            Result
          </div>
          {Object.entries(job.resultSummary).map(([key, val]) => (
            <div key={key} className="flex justify-between py-0.5 text-[12px]">
              <span className="text-[#8787af]">{key}</span>
              <span className="max-w-[60%] truncate tabular-nums text-[#afd7ff]">
                {summarizeStructuredValue(val)}
              </span>
            </div>
          ))}
        </div>
      )}

      {/* Action buttons */}
      <div className="flex flex-wrap gap-2">
        {canCancel && (
          <ActionButton
            label={cancelSent ? 'Canceling...' : 'Cancel'}
            onClick={handleCancel}
            variant="danger"
            disabled={cancelSent}
          />
        )}
        {!isTerminal && (
          <ActionButton label="Check Status" onClick={handleCheckStatus} variant="default" />
        )}
        {job.status === 'failed' && (
          <ActionButton label="View Errors" onClick={handleViewErrors} variant="muted" />
        )}
      </div>
    </div>
  )
}

function ActionButton({
  label,
  onClick,
  variant,
  disabled,
}: {
  label: string
  onClick: () => void
  variant: 'danger' | 'default' | 'muted'
  disabled?: boolean
}) {
  const variantClasses =
    variant === 'danger'
      ? 'border-[rgba(255,95,135,0.3)] text-[#ff5f87] hover:bg-[rgba(255,95,135,0.1)]'
      : variant === 'muted'
        ? 'border-[rgba(135,135,175,0.3)] text-[#8787af] hover:bg-[rgba(135,135,175,0.1)]'
        : 'border-[rgba(135,175,255,0.3)] text-[#87afff] hover:bg-[rgba(135,175,255,0.1)]'

  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`rounded-md border px-3 py-1.5 text-[11px] font-medium transition-colors duration-150 disabled:opacity-40 ${variantClasses}`}
    >
      {label}
    </button>
  )
}

function EmptyState({ isProcessing }: { isProcessing: boolean }) {
  return (
    <div className="flex h-32 items-center justify-center">
      {isProcessing ? (
        <div className="flex items-center gap-2 text-[13px] text-[#8787af]">
          <span className="inline-block size-2 animate-pulse rounded-full bg-[#ffaf87] shadow-[0_0_8px_rgba(255,175,135,0.6)]" />
          <span>Enqueuing job...</span>
        </div>
      ) : (
        <span className="text-[13px] text-[#8787af]">No job data available</span>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main renderer
// ---------------------------------------------------------------------------

export function JobLifecycleRenderer({
  stdoutJson,
  commandMode,
  isProcessing,
  errorMessage,
}: JobLifecycleRendererProps) {
  const { subscribe } = useAxonWs()
  const [polledUpdates, setPolledUpdates] = useState<Record<string, unknown>[]>([])

  // Listen for stdout_json that come from backend polling (status updates)
  useEffect(() => {
    return subscribe((msg: WsServerMsg) => {
      if (msg.type === 'stdout_json' && isRecord(msg.data)) {
        const data = msg.data as Record<string, unknown>
        if (data.job_id || data.id) {
          setPolledUpdates((prev) => {
            const next = [...prev, data]
            return next.length > 100 ? next.slice(-100) : next
          })
        }
      }
    })
  }, [subscribe])

  // Merge initial stdoutJson with polled updates, latest per job_id wins.
  // polledUpdates are appended after stdoutJson so iterating in order
  // ensures the most recent state overwrites earlier entries naturally.
  const jobs = useMemo(() => {
    const allItems = [...stdoutJson, ...polledUpdates]
    const jobMap = new Map<string, JobState>()

    for (const item of allItems) {
      const state = extractJobState(item)
      if (!state) continue
      jobMap.set(state.jobId, state)
    }

    return Array.from(jobMap.values())
  }, [stdoutJson, polledUpdates])

  // Show top-level error from the command itself (not a job error)
  if (errorMessage && jobs.length === 0) {
    return (
      <div className="rounded-md border border-[rgba(255,95,135,0.2)] bg-[rgba(255,95,135,0.06)] px-4 py-3">
        <div className="mb-1 text-[11px] font-bold uppercase tracking-wider text-[#ff5f87]">
          Error
        </div>
        <div className="font-mono text-[13px] leading-relaxed text-[#dce6f0]">{errorMessage}</div>
      </div>
    )
  }

  if (jobs.length === 0) {
    return <EmptyState isProcessing={isProcessing} />
  }

  return (
    <div className="space-y-3">
      {/* Summary header when multiple jobs */}
      {jobs.length > 1 && (
        <div className="flex items-center gap-3 text-[11px] text-[#8787af]">
          <span>{jobs.length} jobs</span>
          <span>&middot;</span>
          <span>{jobs.filter((j) => j.status === 'completed').length} completed</span>
          {jobs.some((j) => j.status === 'running') && (
            <>
              <span>&middot;</span>
              <span className="text-[#87afff]">
                {jobs.filter((j) => j.status === 'running').length} running
              </span>
            </>
          )}
          {jobs.some((j) => j.status === 'failed') && (
            <>
              <span>&middot;</span>
              <span className="text-[#ff5f87]">
                {jobs.filter((j) => j.status === 'failed').length} failed
              </span>
            </>
          )}
        </div>
      )}

      {jobs.map((job) => (
        <JobCard key={job.jobId} job={job} commandMode={commandMode} />
      ))}
    </div>
  )
}
