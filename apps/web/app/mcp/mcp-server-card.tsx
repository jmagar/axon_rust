'use client'

import { Globe, Pencil, Terminal, Trash2 } from 'lucide-react'

import type { McpServerConfig, McpServerStatus } from './mcp-types'

// ── McpServerCard ──────────────────────────────────────────────────────────────

const STATUS_DOT: Record<McpServerStatus, string> = {
  online: 'bg-green-400 shadow-[0_0_6px_rgba(74,222,128,0.7)]',
  offline: 'bg-red-400',
  unknown: 'bg-[rgba(255,255,255,0.2)]',
  checking: 'bg-[var(--axon-primary)] animate-pulse',
}

const STATUS_LABEL: Record<McpServerStatus, string> = {
  online: 'online',
  offline: 'offline',
  unknown: 'unknown',
  checking: 'checking…',
}

export function McpServerCard({
  name,
  cfg,
  status = 'unknown',
  onEdit,
  onDelete,
}: {
  name: string
  cfg: McpServerConfig
  status?: McpServerStatus
  onEdit: () => void
  onDelete: () => void
}) {
  const isHttp = Boolean(cfg.url)
  return (
    <div className="flex items-start justify-between gap-4 rounded-xl border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.38)] px-4 py-3.5 transition-all duration-150 hover:border-[var(--border-standard)] hover:bg-[rgba(10,18,35,0.55)]">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          {/* Status dot */}
          <span
            className={`inline-block size-2 shrink-0 rounded-full ${STATUS_DOT[status]}`}
            title={STATUS_LABEL[status]}
          />
          <span className="text-[13px] font-semibold text-[var(--text-primary)]">{name}</span>
          <span
            className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[9px] font-semibold uppercase tracking-wider ${
              isHttp
                ? 'border border-[rgba(175,215,255,0.2)] bg-[rgba(175,215,255,0.07)] text-[rgba(175,215,255,0.6)]'
                : 'border border-[rgba(255,135,175,0.2)] bg-[rgba(255,135,175,0.07)] text-[rgba(255,135,175,0.7)]'
            }`}
          >
            {isHttp ? <Globe className="size-2.5" /> : <Terminal className="size-2.5" />}
            {isHttp ? 'http' : 'stdio'}
          </span>
          <span
            className={`text-[10px] ${status === 'online' ? 'text-green-400' : status === 'offline' ? 'text-red-400' : 'text-[var(--text-dim)]'}`}
          >
            {STATUS_LABEL[status]}
          </span>
        </div>
        <p className="mt-1 truncate font-mono text-[11px] text-[var(--text-dim)]">
          {isHttp ? cfg.url : cfg.command}
        </p>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <button
          type="button"
          onClick={onEdit}
          className="rounded-md p-1.5 text-[var(--text-dim)] transition-colors hover:bg-[rgba(175,215,255,0.1)] hover:text-[var(--axon-primary-strong)]"
          aria-label={`Edit ${name}`}
        >
          <Pencil className="size-3.5" />
        </button>
        <button
          type="button"
          onClick={onDelete}
          className="rounded-md p-1.5 text-[var(--text-dim)] transition-colors hover:bg-[rgba(255,80,80,0.12)] hover:text-red-400"
          aria-label={`Delete ${name}`}
        >
          <Trash2 className="size-3.5" />
        </button>
      </div>
    </div>
  )
}
