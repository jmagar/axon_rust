'use client'

import { ArrowLeft, CalendarClock, Plus, RefreshCw, Trash2 } from 'lucide-react'
import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useState } from 'react'
import type { Task, TaskRun } from '@/app/api/tasks/route'
import { TaskForm } from './task-form'
import { TasksList } from './tasks-list'

// ── Types ──────────────────────────────────────────────────────────────────────

type FormMode = { kind: 'new' } | { kind: 'edit'; task: Task } | null

interface TasksResponse {
  tasks: Task[]
}

interface RunResponse {
  runId: string
}

// ── API helpers ────────────────────────────────────────────────────────────────

async function apiJson<T>(url: string, opts?: RequestInit): Promise<T> {
  const res = await fetch(url, opts)
  const data = (await res.json()) as T & { error?: string }
  if (!res.ok) throw new Error((data as { error?: string }).error ?? `HTTP ${res.status}`)
  return data
}

// ── Component ─────────────────────────────────────────────────────────────────

export function TasksDashboard() {
  const router = useRouter()
  const [tasks, setTasks] = useState<Task[]>([])
  const [lastRuns, setLastRuns] = useState<Record<string, TaskRun | undefined>>({})
  const [runningIds, setRunningIds] = useState<Set<string>>(new Set())
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [formMode, setFormMode] = useState<FormMode>(null)
  const [saving, setSaving] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<Task | null>(null)
  const [spinning, setSpinning] = useState(false)

  const fetchTasks = useCallback(async (signal?: AbortSignal) => {
    try {
      const data = await apiJson<TasksResponse>('/api/tasks', { signal })
      setTasks(data.tasks)
      setError(null)

      // For each task, fetch most recent run to populate lastRuns map
      const runMap: Record<string, TaskRun | undefined> = {}
      await Promise.all(
        data.tasks.map(async (task) => {
          try {
            const detail = await apiJson<{ task: Task; runs: TaskRun[] }>(
              `/api/tasks?id=${task.id}`,
              { signal },
            )
            runMap[task.id] = detail.runs[0]
          } catch {
            runMap[task.id] = undefined
          }
        }),
      )
      setLastRuns(runMap)
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') return
      setError(err instanceof Error ? err.message : 'Failed to load tasks')
    } finally {
      setLoading(false)
      setSpinning(false)
    }
  }, [])

  useEffect(() => {
    const controller = new AbortController()
    void fetchTasks(controller.signal)
    return () => controller.abort()
  }, [fetchTasks])

  function handleRefresh() {
    setSpinning(true)
    setLoading(true)
    void fetchTasks()
  }

  async function handleSave(data: Omit<Task, 'id' | 'createdAt' | 'updatedAt'>) {
    setSaving(true)
    try {
      if (formMode?.kind === 'edit') {
        const updated = await apiJson<{ task: Task }>('/api/tasks', {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ ...formMode.task, ...data }),
        })
        setTasks((prev) => prev.map((t) => (t.id === updated.task.id ? updated.task : t)))
      } else {
        const created = await apiJson<{ task: Task }>('/api/tasks', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(data),
        })
        setTasks((prev) => [...prev, created.task])
      }
      setFormMode(null)
    } finally {
      setSaving(false)
    }
  }

  async function handleDelete(task: Task) {
    try {
      await apiJson<{ ok: boolean }>(`/api/tasks?id=${task.id}`, { method: 'DELETE' })
      setTasks((prev) => prev.filter((t) => t.id !== task.id))
      setDeleteTarget(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Delete failed')
    }
  }

  async function handleRun(task: Task) {
    setRunningIds((prev) => new Set(prev).add(task.id))
    try {
      await apiJson<RunResponse>(`/api/tasks/run?id=${task.id}`, { method: 'POST' })
      // Poll for completion — simplified: re-fetch after 3s
      setTimeout(() => {
        setRunningIds((prev) => {
          const s = new Set(prev)
          s.delete(task.id)
          return s
        })
        void fetchTasks()
      }, 3000)
    } catch (err) {
      setRunningIds((prev) => {
        const s = new Set(prev)
        s.delete(task.id)
        return s
      })
      setError(err instanceof Error ? err.message : 'Run failed')
    }
  }

  async function handleToggle(task: Task) {
    try {
      const updated = await apiJson<{ task: Task }>('/api/tasks', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...task, enabled: !task.enabled }),
      })
      setTasks((prev) => prev.map((t) => (t.id === updated.task.id ? updated.task : t)))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Toggle failed')
    }
  }

  return (
    <div
      className="flex min-h-dvh flex-col"
      style={{
        background:
          'radial-gradient(ellipse at 14% 10%, rgba(175,215,255,0.08), transparent 34%), radial-gradient(ellipse at 82% 16%, rgba(255,135,175,0.07), transparent 38%), linear-gradient(180deg,#02040b 0%,#030712 60%,#040a14 100%)',
      }}
    >
      {/* Header */}
      <header
        className="sticky top-0 z-30 flex shrink-0 items-center gap-3 border-b px-4"
        style={{
          borderColor: 'var(--border-subtle)',
          background: 'rgba(3,7,18,0.9)',
          backdropFilter: 'blur(16px)',
          height: '3.25rem',
        }}
      >
        <button
          type="button"
          onClick={() => router.back()}
          className="flex min-h-[44px] items-center gap-1.5 rounded-md px-2 py-1 text-[12px] font-medium text-[var(--text-dim)] transition-colors hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)] sm:min-h-0"
          aria-label="Go back"
        >
          <ArrowLeft className="size-3.5" />
          Back
        </button>
        <div className="h-4 w-px bg-[var(--border-subtle)]" />
        <div className="flex items-center gap-2">
          <CalendarClock className="size-3.5 text-[var(--axon-primary-strong)]" />
          <h1 className="text-[14px] font-semibold text-[var(--text-primary)]">Tasks</h1>
        </div>
        <div className="flex-1" />
        <button
          type="button"
          onClick={handleRefresh}
          disabled={loading}
          className="flex min-h-[44px] items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-medium text-[var(--text-dim)] transition-colors hover:bg-[var(--surface-float)] hover:text-[var(--axon-primary)] disabled:opacity-40 sm:min-h-0"
          title="Refresh"
        >
          <RefreshCw className={`size-3 ${spinning ? 'animate-spin' : ''}`} />
          Refresh
        </button>
        <button
          type="button"
          onClick={() => setFormMode({ kind: 'new' })}
          className="flex items-center gap-1.5 rounded-lg border border-[rgba(175,215,255,0.18)] bg-[rgba(175,215,255,0.07)] px-3 py-1.5 text-[12px] font-semibold text-[var(--axon-primary-strong)] transition-colors hover:bg-[rgba(175,215,255,0.13)]"
        >
          <Plus className="size-3.5" />
          New Task
        </button>
      </header>

      {/* Main */}
      <main className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-5xl px-4 py-8 sm:px-6">
          {/* Global error */}
          {error && (
            <div className="mb-4 rounded-xl border border-[rgba(255,80,80,0.2)] bg-[rgba(255,80,80,0.08)] px-4 py-3 text-[13px] text-red-400">
              {error}
              <button
                type="button"
                onClick={() => setError(null)}
                className="ml-3 underline opacity-70 hover:opacity-100"
              >
                dismiss
              </button>
            </div>
          )}

          {/* New task form */}
          {formMode?.kind === 'new' && (
            <div className="mb-6">
              <TaskForm
                key="new"
                onSave={handleSave}
                onCancel={() => setFormMode(null)}
                saving={saving}
              />
            </div>
          )}

          {/* Loading */}
          {loading ? (
            <div className="flex items-center justify-center py-20">
              <div className="size-6 animate-spin rounded-full border-2 border-[rgba(175,215,255,0.2)] border-t-[var(--axon-primary-strong)]" />
            </div>
          ) : (
            <>
              <TasksList
                tasks={tasks}
                runs={lastRuns}
                runningIds={runningIds}
                onEdit={(task) => setFormMode({ kind: 'edit', task })}
                onDelete={(task) => setDeleteTarget(task)}
                onRun={(task) => {
                  void handleRun(task)
                }}
                onToggle={(task) => {
                  void handleToggle(task)
                }}
              />

              {/* Inline edit form */}
              {formMode?.kind === 'edit' && (
                <div className="mt-4">
                  <TaskForm
                    key={formMode.task.id}
                    initial={formMode.task}
                    onSave={handleSave}
                    onCancel={() => setFormMode(null)}
                    saving={saving}
                  />
                </div>
              )}
            </>
          )}

          <div className="h-16" />
        </div>
      </main>

      {/* Delete confirmation modal */}
      {deleteTarget && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-[rgba(3,7,18,0.75)] backdrop-blur-sm animate-fade-in">
          <div className="w-full max-w-sm rounded-xl border border-[var(--border-standard)] bg-[var(--surface-base)] p-5 shadow-[var(--shadow-xl)] animate-scale-in">
            <div className="mb-1 flex items-center gap-2">
              <Trash2 className="size-4 text-[var(--axon-secondary)]" />
              <h3 className="font-display text-sm font-semibold text-[var(--text-primary)]">
                Delete &ldquo;{deleteTarget.name}&rdquo;?
              </h3>
            </div>
            <p className="mb-4 text-xs text-[var(--text-muted)]">
              This task and its run history will be permanently removed.
            </p>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setDeleteTarget(null)}
                className="rounded-md border border-[var(--border-subtle)] bg-transparent px-3 py-1.5 text-xs text-[var(--text-secondary)] transition-colors hover:bg-[var(--surface-float)]"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => {
                  void handleDelete(deleteTarget)
                }}
                className="rounded-md border border-[var(--border-accent)] bg-[rgba(255,135,175,0.15)] px-3 py-1.5 text-xs text-[var(--axon-secondary)] transition-colors hover:bg-[rgba(255,135,175,0.25)]"
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
