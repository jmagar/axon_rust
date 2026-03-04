'use client'

import { ArrowLeft, Bot, RefreshCw } from 'lucide-react'
import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useState } from 'react'
import type { Agent } from '@/app/api/agents/route'
import { ErrorBoundary } from '@/components/ui/error-boundary'
import { apiFetch } from '@/lib/api-fetch'

// ── Types ─────────────────────────────────────────────────────────────────────

interface AgentsResponse {
  agents: Agent[]
  groups: string[]
  error?: string
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function truncateGroupPath(source: string): string {
  const match = source.match(/^(.*?)\s*\((.+)\)$/)
  if (!match) return source
  const label = match[1].trim()
  const rawPath = match[2]
  const parts = rawPath.split('/').filter(Boolean)
  const short = parts.length > 2 ? `…/${parts.slice(-2).join('/')}` : rawPath
  return `${label} (${short})`
}

function sourceBadgeStyle(source: string): { bg: string; color: string; label: string } {
  const lower = source.toLowerCase()
  if (lower === 'built-in') {
    return { bg: 'rgba(175,215,255,0.15)', color: '#afd7ff', label: 'Built-in' }
  }
  if (lower.startsWith('configured') || lower.startsWith('project')) {
    return { bg: 'rgba(255,135,175,0.15)', color: '#ff87af', label: 'Project' }
  }
  return { bg: 'rgba(93,135,175,0.15)', color: '#93aaca', label: 'Global' }
}

// ── Sub-components ─────────────────────────────────────────────────────────────

function SkeletonCard() {
  return (
    <div
      className="rounded-xl border p-4"
      style={{
        background: 'rgba(10,18,35,0.55)',
        backdropFilter: 'blur(12px)',
        borderColor: 'var(--border-subtle)',
      }}
    >
      <div
        className="mb-2 h-4 w-2/3 animate-shimmer rounded"
        style={{
          background:
            'linear-gradient(90deg, rgba(255,135,175,0.07) 25%, rgba(255,135,175,0.13) 50%, rgba(255,135,175,0.07) 75%)',
          backgroundSize: '200% 100%',
        }}
      />
      <div
        className="h-3 w-full animate-shimmer rounded"
        style={{
          background:
            'linear-gradient(90deg, rgba(255,135,175,0.05) 25%, rgba(255,135,175,0.09) 50%, rgba(255,135,175,0.05) 75%)',
          backgroundSize: '200% 100%',
        }}
      />
      <div
        className="mt-1 h-3 w-4/5 animate-shimmer rounded"
        style={{
          background:
            'linear-gradient(90deg, rgba(255,135,175,0.05) 25%, rgba(255,135,175,0.09) 50%, rgba(255,135,175,0.05) 75%)',
          backgroundSize: '200% 100%',
        }}
      />
    </div>
  )
}

function AgentCard({ agent }: { agent: Agent }) {
  const badge = sourceBadgeStyle(agent.source)

  return (
    <article
      className="relative rounded-xl border p-4 transition-all duration-200 hover:border-[rgba(175,215,255,0.25)] hover:shadow-[0_0_20px_rgba(175,215,255,0.06)]"
      style={{
        background: 'rgba(10,18,35,0.55)',
        backdropFilter: 'blur(12px)',
        borderColor: 'var(--border-subtle)',
      }}
    >
      <div className="mb-2 flex items-start justify-between gap-2">
        <span
          className="text-[13px] font-bold leading-snug text-[var(--text-primary)]"
          style={{ fontFamily: 'var(--font-mono)' }}
        >
          {agent.name}
        </span>
        <span
          className="shrink-0 rounded-full px-2 py-0.5 text-[9px] font-semibold uppercase tracking-widest"
          style={{ background: badge.bg, color: badge.color }}
        >
          {badge.label}
        </span>
      </div>
      <p className="text-[11px] leading-relaxed text-[var(--text-dim)]">{agent.description}</p>
    </article>
  )
}

function GroupSection({ source, agents }: { source: string; agents: Agent[] }) {
  const displayName = truncateGroupPath(source)
  return (
    <section className="mb-8">
      <div className="mb-3 flex items-center gap-3">
        <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--text-dim)]">
          {displayName}
        </p>
        <span className="text-[10px] text-[var(--text-dim)]">
          {agents.length} {agents.length === 1 ? 'agent' : 'agents'}
        </span>
        <div className="h-px flex-1 bg-[var(--border-subtle)]" />
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {agents.map((agent) => (
          <AgentCard key={`${agent.source}-${agent.name}`} agent={agent} />
        ))}
      </div>
    </section>
  )
}

// ── Page ───────────────────────────────────────────────────────────────────────

function AgentsPageInner() {
  const router = useRouter()
  const [agents, setAgents] = useState<Agent[]>([])
  const [groups, setGroups] = useState<string[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [spinning, setSpinning] = useState(false)

  const fetchAgents = useCallback(async (signal?: AbortSignal) => {
    setLoading(true)
    setError(null)
    try {
      const res = await apiFetch('/api/agents', { signal })
      const data = (await res.json()) as AgentsResponse
      if (data.error) {
        setError(data.error)
        setAgents([])
        setGroups([])
      } else {
        setAgents(data.agents)
        setGroups(data.groups)
      }
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') return
      setError(err instanceof Error ? err.message : 'Failed to fetch agents')
      setAgents([])
      setGroups([])
    } finally {
      setLoading(false)
      setSpinning(false)
    }
  }, [])

  useEffect(() => {
    const controller = new AbortController()
    void fetchAgents(controller.signal)
    return () => controller.abort()
  }, [fetchAgents])

  function handleRefresh() {
    setSpinning(true)
    void fetchAgents()
  }

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
          <Bot className="size-3.5 text-[var(--axon-primary-strong)]" />
          <h1 className="text-[14px] font-semibold text-[var(--text-primary)]">Agents</h1>
        </div>
        <div className="flex-1" />
        <button
          type="button"
          onClick={handleRefresh}
          disabled={loading}
          className="flex min-h-[44px] items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-medium text-[var(--text-dim)] transition-colors hover:bg-[var(--surface-float)] hover:text-[var(--axon-primary)] disabled:opacity-40 sm:min-h-0"
          title="Refresh agent list"
        >
          <RefreshCw className={`size-3 ${spinning ? 'animate-spin' : ''}`} />
          Refresh
        </button>
      </header>

      {/* Main content */}
      <main className="relative z-10 flex-1">
        <div className="mx-auto max-w-[960px] px-4 py-8 sm:px-6">
          {/* Loading state */}
          {loading && (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
              <SkeletonCard />
              <SkeletonCard />
              <SkeletonCard />
            </div>
          )}

          {/* Error state */}
          {!loading && error && (
            <div
              className="rounded-xl border px-6 py-8 text-center"
              style={{
                background: 'rgba(10,18,35,0.55)',
                backdropFilter: 'blur(12px)',
                borderColor: 'var(--border-accent)',
              }}
            >
              <Bot className="mx-auto mb-3 size-8 text-[var(--text-dim)]" />
              <p className="mb-1 text-[13px] font-medium text-[var(--text-secondary)]">
                Could not load agents
              </p>
              <p className="text-[11px] leading-relaxed text-[var(--text-dim)]">{error}</p>
            </div>
          )}

          {/* Empty state */}
          {!loading && !error && agents.length === 0 && (
            <div className="flex flex-col items-center justify-center gap-3 py-12 text-center animate-fade-in">
              <Bot className="size-8 text-[var(--text-dim)]" />
              <div>
                <p className="text-sm font-medium text-[var(--text-secondary)]">No agents found</p>
                <p className="text-xs text-[var(--text-muted)] mt-1">
                  Run{' '}
                  <code className="rounded border border-[var(--border-subtle)] px-1 text-[var(--axon-primary)]">
                    claude agents
                  </code>{' '}
                  in your terminal to verify the CLI is configured.
                </p>
              </div>
            </div>
          )}

          {/* Agent groups */}
          {!loading && !error && agents.length > 0 && (
            <div>
              {groups.map((source) => {
                const groupAgents = agents.filter((a) => a.source === source)
                if (groupAgents.length === 0) return null
                return <GroupSection key={source} source={source} agents={groupAgents} />
              })}
            </div>
          )}

          <div className="h-16" />
        </div>
      </main>
    </div>
  )
}

export default function AgentsPage() {
  return (
    <ErrorBoundary>
      <AgentsPageInner />
    </ErrorBoundary>
  )
}
