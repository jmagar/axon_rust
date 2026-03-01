'use client'

import { Pencil, Play, Trash2 } from 'lucide-react'
import type { Task, TaskRun } from '@/app/api/tasks/route'

// ── Types ──────────────────────────────────────────────────────────────────────

interface TasksListProps {
  tasks: Task[]
  runs: Record<string, TaskRun | undefined>
  runningIds: Set<string>
  onEdit: (task: Task) => void
  onDelete: (task: Task) => void
  onRun: (task: Task) => void
  onToggle: (task: Task) => void
}

// ── Helpers ────────────────────────────────────────────────────────────────────

function formatSchedule(schedule: string): string {
  if (schedule === 'once') return 'One-off'
  return schedule
}

function formatRelativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime()
  const mins = Math.floor(diff / 60_000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins}m ago`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  return `${days}d ago`
}

function RunStatusDot({ status }: { status: TaskRun['status'] }) {
  const colors = {
    running: 'bg-[var(--axon-primary)]',
    completed: 'bg-[var(--axon-success)]',
    failed: 'bg-[var(--axon-secondary)]',
  }
  return (
    <span
      className={`inline-block size-1.5 rounded-full ${colors[status]} ${status === 'running' ? 'animate-pulse' : ''}`}
      title={status}
    />
  )
}

// ── Component ─────────────────────────────────────────────────────────────────

export function TasksList({
  tasks,
  runs,
  runningIds,
  onEdit,
  onDelete,
  onRun,
  onToggle,
}: TasksListProps) {
  if (tasks.length === 0) {
    return (
      <div className="flex min-h-[200px] flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-[var(--border-subtle)] bg-[var(--surface-float)] p-8 text-center animate-fade-in">
        <p className="text-[13px] font-medium text-[var(--text-secondary)]">No tasks yet</p>
        <p className="text-[11px] text-[var(--text-dim)]">
          Click <strong className="text-[var(--text-muted)]">+ New Task</strong> to schedule your
          first command.
        </p>
      </div>
    )
  }

  return (
    <div
      className="overflow-hidden rounded-xl border"
      style={{
        borderColor: 'var(--border-subtle)',
        background: 'rgba(10,18,35,0.55)',
        backdropFilter: 'blur(12px)',
      }}
    >
      <table className="ui-table-dense w-full">
        <thead>
          <tr>
            <th className="ui-table-head px-4 py-2.5 text-left">Name</th>
            <th className="ui-table-head px-4 py-2.5 text-left">Schedule</th>
            <th className="ui-table-head px-4 py-2.5 text-left">Status</th>
            <th className="ui-table-head px-4 py-2.5 text-left">Last Run</th>
            <th className="ui-table-head px-4 py-2.5 text-right">Actions</th>
          </tr>
        </thead>
        <tbody>
          {tasks.map((task, i) => {
            const lastRun = runs[task.id]
            const isRunning = runningIds.has(task.id)
            const rowBorder = i < tasks.length - 1 ? '1px solid rgba(135,175,255,0.06)' : 'none'

            return (
              <tr
                key={task.id}
                className="group transition-colors hover:bg-[rgba(135,175,255,0.04)]"
                style={{ borderBottom: rowBorder }}
              >
                {/* Name */}
                <td className="ui-table-cell px-4 py-2.5">
                  <div className="flex flex-col gap-0.5">
                    <span className="text-[13px] font-medium text-[var(--text-primary)]">
                      {task.name}
                    </span>
                    {task.description && (
                      <span className="text-[11px] text-[var(--text-dim)]">{task.description}</span>
                    )}
                    <span
                      className="max-w-[240px] truncate text-[10px] text-[var(--text-dim)]"
                      style={{ fontFamily: 'var(--font-mono)' }}
                      title={task.command}
                    >
                      {task.command}
                    </span>
                  </div>
                </td>

                {/* Schedule */}
                <td className="ui-table-cell px-4 py-2.5">
                  <span
                    className="text-[12px] text-[var(--text-secondary)]"
                    style={{ fontFamily: 'var(--font-mono)' }}
                  >
                    {formatSchedule(task.schedule)}
                  </span>
                </td>

                {/* Status toggle */}
                <td className="ui-table-cell px-4 py-2.5">
                  <button
                    type="button"
                    onClick={() => onToggle(task)}
                    className={`ui-chip-status transition-colors hover:opacity-80 ${
                      task.enabled
                        ? 'bg-[rgba(130,217,160,0.12)] text-[var(--axon-success)]'
                        : 'bg-[rgba(93,135,175,0.1)] text-[var(--text-dim)]'
                    }`}
                    title={task.enabled ? 'Click to disable' : 'Click to enable'}
                  >
                    <span
                      className={`inline-block size-1.5 rounded-full ${task.enabled ? 'bg-[var(--axon-success)]' : 'bg-[var(--text-dim)]'}`}
                    />
                    {task.enabled ? 'Enabled' : 'Disabled'}
                  </button>
                </td>

                {/* Last run */}
                <td className="ui-table-cell px-4 py-2.5">
                  {isRunning ? (
                    <div className="flex items-center gap-1.5">
                      <RunStatusDot status="running" />
                      <span className="text-[12px] text-[var(--axon-primary)]">Running…</span>
                    </div>
                  ) : lastRun ? (
                    <div className="flex items-center gap-1.5">
                      <RunStatusDot status={lastRun.status} />
                      <span className="text-[12px] text-[var(--text-secondary)]">
                        {formatRelativeTime(lastRun.startedAt)}
                      </span>
                      <span className="text-[10px] text-[var(--text-dim)]">({lastRun.status})</span>
                    </div>
                  ) : (
                    <span className="text-[12px] text-[var(--text-dim)]">Never</span>
                  )}
                </td>

                {/* Actions */}
                <td className="ui-table-cell px-4 py-2.5 text-right">
                  <div className="flex items-center justify-end gap-1">
                    <button
                      type="button"
                      onClick={() => onRun(task)}
                      disabled={isRunning}
                      className="rounded-md p-1.5 text-[var(--text-dim)] transition-colors hover:bg-[rgba(175,215,255,0.1)] hover:text-[var(--axon-primary)] disabled:opacity-30"
                      title="Run now"
                    >
                      <Play className="size-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => onEdit(task)}
                      className="rounded-md p-1.5 text-[var(--text-dim)] transition-colors hover:bg-[rgba(175,215,255,0.1)] hover:text-[var(--text-secondary)]"
                      title="Edit"
                    >
                      <Pencil className="size-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => onDelete(task)}
                      className="rounded-md p-1.5 text-[var(--text-dim)] transition-colors hover:bg-[rgba(255,135,175,0.1)] hover:text-[var(--axon-secondary)]"
                      title="Delete"
                    >
                      <Trash2 className="size-3.5" />
                    </button>
                  </div>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
