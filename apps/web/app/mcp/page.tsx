'use client'

import { ArrowLeft, Network, Plus, Trash2 } from 'lucide-react'
import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useState } from 'react'
import { ErrorBoundary } from '@/components/ui/error-boundary'
import {
  configToForm,
  EMPTY_FORM,
  type FormState,
  type McpConfig,
  McpServerCard,
  type McpServerConfig,
  McpServerForm,
  type McpServerStatus,
} from './components'

// ── Page ───────────────────────────────────────────────────────────────────────

function McpPageInner() {
  const router = useRouter()
  const [config, setConfig] = useState<McpConfig>({ mcpServers: {} })
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [formOpen, setFormOpen] = useState(false)
  const [editTarget, setEditTarget] = useState<string | null>(null)
  const [deleteModal, setDeleteModal] = useState<{ name: string } | null>(null)
  const [statusMap, setStatusMap] = useState<Record<string, McpServerStatus>>({})

  const loadStatus = useCallback(async (signal?: AbortSignal) => {
    try {
      const res = await fetch('/api/mcp/status', { signal })
      if (!res.ok) return
      const data = (await res.json()) as { servers: Record<string, McpServerStatus> }
      setStatusMap(data.servers)
    } catch (err) {
      // Status check failure is non-critical — don't surface as error.
      // AbortError is expected on cleanup; silently ignore all status errors.
      void err
    }
  }, [])

  const loadConfig = useCallback(
    async (signal?: AbortSignal) => {
      try {
        const res = await fetch('/api/mcp', { signal })
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        const data = (await res.json()) as McpConfig
        setConfig(data)
        setError('')
        // Mark all servers as checking while we probe them
        setStatusMap(
          Object.fromEntries(
            Object.keys(data.mcpServers).map((k) => [k, 'checking' as McpServerStatus]),
          ),
        )
        // Kick off status check after config loads
        void loadStatus(signal)
      } catch (err) {
        if (err instanceof Error && err.name === 'AbortError') return
        setError(err instanceof Error ? err.message : 'Failed to load')
      } finally {
        setLoading(false)
      }
    },
    [loadStatus],
  )

  useEffect(() => {
    const controller = new AbortController()
    void loadConfig(controller.signal)
    return () => controller.abort()
  }, [loadConfig])

  async function saveServer(name: string, cfg: McpServerConfig) {
    // Capture both the previous state (for rollback) and the merged state (for the PUT).
    // Using the functional updater form so we always read the latest prev value.
    let previousConfig: McpConfig = { mcpServers: {} }
    let mergedConfig: McpConfig = { mcpServers: {} }
    setConfig((prev) => {
      previousConfig = prev
      mergedConfig = { mcpServers: { ...prev.mcpServers, [name]: cfg } }
      return mergedConfig
    })
    setFormOpen(false)
    setEditTarget(null)
    // mergedConfig is synchronously assigned inside the setter before this line runs
    // because React batches state updates but calls the updater synchronously.
    const res = await fetch('/api/mcp', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json', 'X-Pulse-Request': '1' },
      body: JSON.stringify(mergedConfig),
    })
    if (!res.ok) {
      // Roll back to the state before the optimistic update so the UI stays
      // consistent with the server.
      setConfig(previousConfig)
      setError('Save failed')
    }
  }

  async function deleteServer(name: string) {
    const res = await fetch('/api/mcp', {
      method: 'DELETE',
      headers: { 'Content-Type': 'application/json', 'X-Pulse-Request': '1' },
      body: JSON.stringify({ name }),
    })
    if (!res.ok) {
      setError('Delete failed')
      return
    }
    const controller = new AbortController()
    await loadConfig(controller.signal)
  }

  function openAdd() {
    setEditTarget(null)
    setFormOpen(true)
  }

  function openEdit(name: string) {
    setEditTarget(name)
    setFormOpen(true)
  }

  function closeForm() {
    setFormOpen(false)
    setEditTarget(null)
  }

  const servers = Object.entries(config.mcpServers)
  const existingNames = servers.map(([n]) => n)

  const formInitial: FormState =
    editTarget && config.mcpServers[editTarget]
      ? configToForm(editTarget, config.mcpServers[editTarget])
      : EMPTY_FORM

  return (
    <div
      className="flex min-h-dvh flex-col"
      style={{
        background:
          'radial-gradient(ellipse at 14% 10%, rgba(175,215,255,0.08), transparent 34%), radial-gradient(ellipse at 82% 16%, rgba(255,135,175,0.07), transparent 38%), linear-gradient(180deg,#02040b 0%,#030712 60%,#040a14 100%)',
      }}
    >
      {/* Top bar */}
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
          <Network className="size-3.5 text-[var(--axon-primary-strong)]" />
          <h1 className="text-[14px] font-semibold text-[var(--text-primary)]">MCP Servers</h1>
        </div>
        <div className="flex-1" />
        <button
          type="button"
          onClick={openAdd}
          className="flex items-center gap-1.5 rounded-lg border border-[rgba(175,215,255,0.18)] bg-[rgba(175,215,255,0.07)] px-3 py-1.5 text-[12px] font-semibold text-[var(--axon-primary-strong)] transition-colors hover:bg-[rgba(175,215,255,0.13)]"
        >
          <Plus className="size-3.5" />
          Add Server
        </button>
      </header>

      {/* Body */}
      <main className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-[720px] px-4 py-8 sm:px-6">
          {error && (
            <div className="mb-4 rounded-xl border border-[rgba(255,80,80,0.2)] bg-[rgba(255,80,80,0.08)] px-4 py-3 text-[13px] text-red-400">
              {error}
            </div>
          )}

          {/* Add / Edit form */}
          {formOpen && (
            <div className="mb-6">
              <McpServerForm
                key={editTarget ?? '__new__'}
                initial={formInitial}
                existingNames={existingNames}
                isEditing={editTarget !== null}
                onSave={saveServer}
                onCancel={closeForm}
              />
            </div>
          )}

          {/* Server list */}
          {loading ? (
            <div className="flex items-center justify-center py-20">
              <div className="size-6 animate-spin rounded-full border-2 border-[rgba(175,215,255,0.2)] border-t-[var(--axon-primary-strong)]" />
            </div>
          ) : servers.length === 0 && !formOpen ? (
            <div className="flex h-full min-h-[300px] flex-col items-center justify-center gap-4 rounded-xl border border-dashed border-[var(--border-subtle)] bg-[var(--surface-float)] p-8 text-center animate-fade-in">
              <div className="relative">
                <div className="absolute inset-0 bg-[radial-gradient(circle,rgba(135,175,255,0.15),transparent)] blur-xl" />
                <Network className="relative size-10 text-[var(--axon-primary)]" />
              </div>
              <div className="space-y-1.5">
                <h3 className="font-display text-sm font-semibold text-[var(--text-primary)]">
                  No MCP servers configured
                </h3>
                <p className="max-w-xs text-xs leading-relaxed text-[var(--text-muted)]">
                  MCP servers extend Claude&apos;s capabilities with external tools, APIs, and data
                  sources.
                </p>
              </div>
              <button
                type="button"
                onClick={openAdd}
                className="mt-1 flex items-center gap-1.5 rounded-lg border border-[var(--border-standard)] bg-[rgba(135,175,255,0.15)] px-4 py-2 text-[12px] font-semibold text-[var(--axon-primary)] transition-colors hover:bg-[rgba(135,175,255,0.25)]"
              >
                <Plus className="size-3.5" />
                Add your first server
              </button>
            </div>
          ) : (
            <div className="space-y-2">
              {servers.map(([name, cfg]) => (
                <McpServerCard
                  key={name}
                  name={name}
                  cfg={cfg}
                  status={statusMap[name] ?? 'unknown'}
                  onEdit={() => openEdit(name)}
                  onDelete={() => setDeleteModal({ name })}
                />
              ))}
            </div>
          )}

          <div className="h-16" />
        </div>
      </main>

      {/* Delete confirmation modal */}
      {deleteModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-[rgba(3,7,18,0.75)] backdrop-blur-sm animate-fade-in">
          <div className="w-full max-w-sm rounded-xl border border-[var(--border-standard)] bg-[var(--surface-base)] p-5 shadow-[var(--shadow-xl)] animate-scale-in">
            <div className="mb-1 flex items-center gap-2">
              <Trash2 className="size-4 text-[var(--axon-secondary)]" />
              <h3 className="font-display text-sm font-semibold text-[var(--text-primary)]">
                Delete &ldquo;{deleteModal.name}&rdquo;?
              </h3>
            </div>
            <p className="mb-4 text-xs text-[var(--text-muted)]">
              This MCP server configuration will be permanently removed. You can add it back later.
            </p>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setDeleteModal(null)}
                className="rounded-md border border-[var(--border-subtle)] bg-transparent px-3 py-1.5 text-xs text-[var(--text-secondary)] hover:bg-[var(--surface-float)] transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => {
                  deleteServer(deleteModal.name)
                  setDeleteModal(null)
                }}
                className="rounded-md bg-[rgba(255,135,175,0.15)] border border-[var(--border-accent)] px-3 py-1.5 text-xs text-[var(--axon-secondary)] hover:bg-[rgba(255,135,175,0.25)] transition-colors"
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

export default function McpPage() {
  return (
    <ErrorBoundary>
      <McpPageInner />
    </ErrorBoundary>
  )
}
