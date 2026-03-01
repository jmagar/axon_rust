'use client'

import { useState } from 'react'
import type { Task } from '@/app/api/tasks/route'

// ── Types ──────────────────────────────────────────────────────────────────────

interface TaskFormProps {
  initial?: Partial<Task>
  onSave: (data: Omit<Task, 'id' | 'createdAt' | 'updatedAt'>) => Promise<void>
  onCancel: () => void
  saving: boolean
}

// ── Styles ─────────────────────────────────────────────────────────────────────

const inputClass =
  'w-full rounded-lg border bg-[rgba(10,18,35,0.6)] px-3 py-2 text-[13px] text-[var(--text-primary)] placeholder:text-[var(--text-dim)] focus:outline-none focus:ring-1 focus:ring-[var(--axon-primary)] transition-colors'

const labelClass =
  'block text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--text-dim)] mb-1'

// ── Component ─────────────────────────────────────────────────────────────────

export function TaskForm({ initial, onSave, onCancel, saving }: TaskFormProps) {
  const [name, setName] = useState(initial?.name ?? '')
  const [description, setDescription] = useState(initial?.description ?? '')
  const [schedule, setSchedule] = useState(initial?.schedule ?? 'once')
  const [command, setCommand] = useState(initial?.command ?? '')
  const [enabled, setEnabled] = useState(initial?.enabled ?? true)
  const [error, setError] = useState<string | null>(null)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    if (!name.trim()) {
      setError('Name is required')
      return
    }
    if (!command.trim()) {
      setError('Command is required')
      return
    }
    if (!schedule.trim()) {
      setError('Schedule is required')
      return
    }
    try {
      await onSave({
        name: name.trim(),
        description: description.trim() || undefined,
        schedule: schedule.trim(),
        command: command.trim(),
        enabled,
      })
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Save failed')
    }
  }

  return (
    <form
      onSubmit={(e) => {
        void handleSubmit(e)
      }}
      className="rounded-xl border p-5 animate-fade-in-up"
      style={{
        background: 'rgba(10,18,35,0.75)',
        backdropFilter: 'blur(12px)',
        borderColor: 'var(--border-standard)',
      }}
    >
      <h3 className="mb-4 text-[13px] font-semibold text-[var(--text-primary)]">
        {initial?.id ? 'Edit Task' : 'New Task'}
      </h3>

      {error && (
        <div className="mb-4 rounded-lg border border-[rgba(255,80,80,0.2)] bg-[rgba(255,80,80,0.08)] px-3 py-2 text-[12px] text-red-400">
          {error}
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        {/* Name */}
        <div className="sm:col-span-2">
          <label htmlFor="tf-name" className={labelClass}>
            Name *
          </label>
          <input
            id="tf-name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Daily docs crawl"
            maxLength={100}
            className={inputClass}
            style={{ borderColor: 'var(--border-subtle)' }}
            required
          />
        </div>

        {/* Description */}
        <div className="sm:col-span-2">
          <label htmlFor="tf-description" className={labelClass}>
            Description
          </label>
          <textarea
            id="tf-description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Optional description"
            rows={2}
            className={`${inputClass} resize-none`}
            style={{ borderColor: 'var(--border-subtle)' }}
          />
        </div>

        {/* Schedule */}
        <div>
          <label htmlFor="tf-schedule" className={labelClass}>
            Schedule *
          </label>
          <input
            id="tf-schedule"
            type="text"
            value={schedule}
            onChange={(e) => setSchedule(e.target.value)}
            placeholder="once  or  0 9 * * *"
            className={inputClass}
            style={{ borderColor: 'var(--border-subtle)' }}
            required
          />
          <p className="mt-1 text-[10px] text-[var(--text-dim)]">
            Use <code className="text-[var(--axon-primary)]">once</code> for one-off or a cron
            expression like <code className="text-[var(--axon-primary)]">0 9 * * *</code> (daily at
            09:00)
          </p>
        </div>

        {/* Command */}
        <div>
          <label htmlFor="tf-command" className={labelClass}>
            Command *
          </label>
          <input
            id="tf-command"
            type="text"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            placeholder="axon crawl https://docs.example.com"
            className={`${inputClass} font-mono`}
            style={{ borderColor: 'var(--border-subtle)', fontFamily: 'var(--font-mono)' }}
            required
          />
          <p className="mt-1 text-[10px] text-[var(--text-dim)]">
            No shell metacharacters (
            <code className="text-[var(--text-muted)]">; | &amp; &gt; &lt; ` $</code>)
          </p>
        </div>

        {/* Enabled */}
        <div className="flex items-center gap-2 sm:col-span-2">
          <input
            id="task-enabled"
            type="checkbox"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
            className="size-4 rounded accent-[var(--axon-primary)]"
          />
          <label
            htmlFor="task-enabled"
            className="text-[12px] text-[var(--text-secondary)] cursor-pointer"
          >
            Enabled
          </label>
        </div>
      </div>

      {/* Actions */}
      <div className="mt-5 flex justify-end gap-2">
        <button
          type="button"
          onClick={onCancel}
          disabled={saving}
          className="rounded-md border border-[var(--border-subtle)] bg-transparent px-4 py-1.5 text-[12px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--surface-float)] disabled:opacity-40"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={saving}
          className="rounded-md border border-[rgba(175,215,255,0.18)] bg-[rgba(175,215,255,0.07)] px-4 py-1.5 text-[12px] font-semibold text-[var(--axon-primary-strong)] transition-colors hover:bg-[rgba(175,215,255,0.13)] disabled:opacity-40"
        >
          {saving ? 'Saving…' : initial?.id ? 'Save Changes' : 'Create Task'}
        </button>
      </div>
    </form>
  )
}
