'use client'

import { Globe, Pencil, Plus, Terminal, Trash2, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

// ── Types ──────────────────────────────────────────────────────────────────────

export type McpServerConfig = {
  command?: string
  args?: string[]
  env?: Record<string, string>
  url?: string
  headers?: Record<string, string>
}

export type McpConfig = {
  mcpServers: Record<string, McpServerConfig>
}

export type ServerType = 'stdio' | 'http'

export type KvPair = { id: string; key: string; value: string }

export type FormState = {
  name: string
  type: ServerType
  command: string
  args: string
  envPairs: KvPair[]
  url: string
  headerPairs: KvPair[]
}

export const EMPTY_FORM: FormState = {
  name: '',
  type: 'stdio',
  command: '',
  args: '',
  envPairs: [],
  url: '',
  headerPairs: [],
}

// ── CSS helpers ────────────────────────────────────────────────────────────────

export const INPUT_CLS =
  'w-full rounded-lg border border-[rgba(255,135,175,0.15)] bg-[rgba(10,18,35,0.5)] px-3 py-2.5 text-[13px] text-[var(--axon-text-secondary)] outline-none placeholder:text-[var(--axon-text-subtle)] focus:border-[rgba(175,215,255,0.35)] focus:bg-[rgba(10,18,35,0.7)]'

export const LABEL_CLS =
  'mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--axon-text-dim)]'

// ── Helpers ────────────────────────────────────────────────────────────────────

export function formToConfig(form: FormState): McpServerConfig {
  if (form.type === 'http') {
    const headers: Record<string, string> = {}
    for (const { key, value } of form.headerPairs) {
      if (key.trim()) headers[key.trim()] = value
    }
    return { url: form.url, ...(Object.keys(headers).length > 0 ? { headers } : {}) }
  }
  const env: Record<string, string> = {}
  for (const { key, value } of form.envPairs) {
    if (key.trim()) env[key.trim()] = value
  }
  const args = form.args
    .split('\n')
    .map((a) => a.trim())
    .filter(Boolean)
  return {
    command: form.command,
    ...(args.length > 0 ? { args } : {}),
    ...(Object.keys(env).length > 0 ? { env } : {}),
  }
}

export function configToForm(name: string, cfg: McpServerConfig): FormState {
  const isHttp = Boolean(cfg.url)
  return {
    name,
    type: isHttp ? 'http' : 'stdio',
    command: cfg.command ?? '',
    args: (cfg.args ?? []).join('\n'),
    envPairs: Object.entries(cfg.env ?? {}).map(([key, value]) => ({
      id: crypto.randomUUID(),
      key,
      value,
    })),
    url: cfg.url ?? '',
    headerPairs: Object.entries(cfg.headers ?? {}).map(([key, value]) => ({
      id: crypto.randomUUID(),
      key,
      value,
    })),
  }
}

// ── KvEditor ───────────────────────────────────────────────────────────────────

export function KvEditor({
  label,
  pairs,
  onChange,
}: {
  label: string
  pairs: KvPair[]
  onChange: (pairs: KvPair[]) => void
}) {
  function addPair() {
    onChange([...pairs, { id: crypto.randomUUID(), key: '', value: '' }])
  }

  function removePair(idx: number) {
    onChange(pairs.filter((_, i) => i !== idx))
  }

  function updatePair(idx: number, field: 'key' | 'value', val: string) {
    onChange(pairs.map((p, i) => (i === idx ? { ...p, [field]: val } : p)))
  }

  return (
    <div>
      <div className="mb-1.5 flex items-center justify-between">
        <span className={LABEL_CLS}>{label}</span>
        <button
          type="button"
          onClick={addPair}
          className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] text-[var(--axon-text-dim)] hover:bg-[rgba(175,215,255,0.08)] hover:text-[var(--axon-accent-pink)]"
        >
          <Plus className="size-3" />
          Add
        </button>
      </div>
      {pairs.length === 0 ? (
        <p className="text-[11px] text-[var(--axon-text-dim)]">None configured.</p>
      ) : (
        <div className="space-y-2">
          {pairs.map((p, i) => (
            <div key={`kv-${p.id}`} className="flex items-center gap-2">
              <input
                type="text"
                value={p.key}
                onChange={(e) => updatePair(i, 'key', e.target.value)}
                placeholder="KEY"
                className="w-2/5 rounded-lg border border-[rgba(255,135,175,0.15)] bg-[rgba(10,18,35,0.5)] px-2.5 py-2 font-mono text-[12px] text-[var(--axon-text-secondary)] outline-none placeholder:text-[var(--axon-text-subtle)] focus:border-[rgba(175,215,255,0.35)]"
              />
              <input
                type="text"
                value={p.value}
                onChange={(e) => updatePair(i, 'value', e.target.value)}
                placeholder="value"
                className="min-w-0 flex-1 rounded-lg border border-[rgba(255,135,175,0.15)] bg-[rgba(10,18,35,0.5)] px-2.5 py-2 text-[12px] text-[var(--axon-text-secondary)] outline-none placeholder:text-[var(--axon-text-subtle)] focus:border-[rgba(175,215,255,0.35)]"
              />
              <button
                type="button"
                onClick={() => removePair(i)}
                className="shrink-0 rounded p-1 text-[var(--axon-text-dim)] hover:bg-[rgba(255,100,100,0.12)] hover:text-red-400"
                aria-label="Remove"
              >
                <X className="size-3.5" />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// ── McpServerCard ──────────────────────────────────────────────────────────────

export type McpServerStatus = 'online' | 'offline' | 'unknown' | 'checking'

const STATUS_DOT: Record<McpServerStatus, string> = {
  online: 'bg-green-400 shadow-[0_0_6px_rgba(74,222,128,0.7)]',
  offline: 'bg-red-400',
  unknown: 'bg-[rgba(255,255,255,0.2)]',
  checking: 'animate-pulse bg-yellow-400',
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
    <div className="flex items-start justify-between gap-4 rounded-xl border border-[rgba(255,135,175,0.1)] bg-[rgba(10,18,35,0.38)] px-4 py-3.5 transition-all duration-150 hover:border-[rgba(255,135,175,0.2)] hover:bg-[rgba(10,18,35,0.55)]">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          {/* Status dot */}
          <span
            className={`inline-block size-2 shrink-0 rounded-full ${STATUS_DOT[status]}`}
            title={STATUS_LABEL[status]}
          />
          <span className="text-[13px] font-semibold text-[var(--axon-text-primary)]">{name}</span>
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
            className={`text-[10px] ${status === 'online' ? 'text-green-400' : status === 'offline' ? 'text-red-400' : 'text-[var(--axon-text-dim)]'}`}
          >
            {STATUS_LABEL[status]}
          </span>
        </div>
        <p className="mt-1 truncate font-mono text-[11px] text-[var(--axon-text-dim)]">
          {isHttp ? cfg.url : cfg.command}
        </p>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <button
          type="button"
          onClick={onEdit}
          className="rounded-md p-1.5 text-[var(--axon-text-dim)] transition-colors hover:bg-[rgba(175,215,255,0.1)] hover:text-[var(--axon-accent-pink)]"
          aria-label={`Edit ${name}`}
        >
          <Pencil className="size-3.5" />
        </button>
        <button
          type="button"
          onClick={onDelete}
          className="rounded-md p-1.5 text-[var(--axon-text-dim)] transition-colors hover:bg-[rgba(255,80,80,0.12)] hover:text-red-400"
          aria-label={`Delete ${name}`}
        >
          <Trash2 className="size-3.5" />
        </button>
      </div>
    </div>
  )
}

// ── McpServerForm ──────────────────────────────────────────────────────────────

export function McpServerForm({
  initial,
  existingNames,
  isEditing,
  onSave,
  onCancel,
}: {
  initial: FormState
  existingNames: string[]
  isEditing: boolean
  onSave: (name: string, cfg: McpServerConfig) => void
  onCancel: () => void
}) {
  const [form, setForm] = useState<FormState>(initial)
  const [activeTab, setActiveTab] = useState<'form' | 'json'>('form')
  const [rawJson, setRawJson] = useState('')
  const [jsonError, setJsonError] = useState('')
  const jsonEditedManuallyRef = useRef(false)

  useEffect(() => {
    if (activeTab === 'json' && !jsonEditedManuallyRef.current) {
      try {
        const config = formToConfig(form)
        const full = { mcpServers: { [form.name || 'server-name']: config } }
        setRawJson(JSON.stringify(full, null, 2))
        setJsonError('')
      } catch {
        // keep existing raw
      }
    }
    if (activeTab !== 'json') {
      jsonEditedManuallyRef.current = false
    }
  }, [activeTab, form])

  function updateField<K extends keyof FormState>(field: K, value: FormState[K]) {
    setForm((prev) => ({ ...prev, [field]: value }))
  }

  function handleSaveJson() {
    try {
      const parsed = JSON.parse(rawJson) as McpConfig
      if (!parsed.mcpServers) throw new Error('Missing mcpServers key')
      const entries = Object.entries(parsed.mcpServers)
      if (entries.length === 0) throw new Error('No servers in mcpServers')
      const [serverName, serverCfg] = entries[0] as [string, McpServerConfig]
      jsonEditedManuallyRef.current = false
      onSave(serverName, serverCfg)
      setJsonError('')
    } catch (err) {
      setJsonError(err instanceof Error ? err.message : 'Invalid JSON')
    }
  }

  function handleSaveForm() {
    if (!form.name.trim()) return
    onSave(form.name.trim(), formToConfig(form))
  }

  const nameConflict = !isEditing && existingNames.includes(form.name.trim())

  return (
    <div
      className="overflow-hidden rounded-xl border border-[rgba(175,215,255,0.18)] bg-[rgba(3,7,18,0.65)] shadow-[0_0_32px_rgba(175,215,255,0.04)]"
      style={{ backdropFilter: 'blur(20px) saturate(180%)' }}
    >
      <div className="flex items-center justify-between border-b border-[rgba(255,135,175,0.1)] px-4 py-3">
        <h3 className="text-[13px] font-semibold text-[var(--axon-text-primary)]">
          {isEditing ? `Edit — ${initial.name}` : 'Add MCP Server'}
        </h3>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-md p-1 text-[var(--axon-text-dim)] hover:bg-[rgba(255,135,175,0.08)] hover:text-[var(--axon-text-secondary)]"
          aria-label="Close"
        >
          <X className="size-4" />
        </button>
      </div>

      {/* Tab bar */}
      <div className="flex border-b border-[rgba(255,135,175,0.08)]">
        {(['form', 'json'] as const).map((tab) => (
          <button
            key={tab}
            type="button"
            onClick={() => setActiveTab(tab)}
            className={`px-4 py-2 text-[12px] font-medium transition-colors ${
              activeTab === tab
                ? 'border-b-2 border-[var(--axon-accent-pink)] text-[var(--axon-accent-pink)]'
                : 'text-[var(--axon-text-dim)] hover:text-[var(--axon-text-secondary)]'
            }`}
          >
            {tab === 'form' ? 'Form' : 'JSON'}
          </button>
        ))}
      </div>

      <div className="p-4">
        {activeTab === 'form' ? (
          <div className="space-y-4">
            <div>
              <label htmlFor="mcp-server-name" className={LABEL_CLS}>
                Server name
              </label>
              <input
                id="mcp-server-name"
                type="text"
                value={form.name}
                onChange={(e) => updateField('name', e.target.value)}
                placeholder="my-server"
                className={INPUT_CLS}
                disabled={isEditing}
              />
              {nameConflict && (
                <p className="mt-1 text-[11px] text-red-400">
                  A server with this name already exists.
                </p>
              )}
            </div>

            <div>
              <span className={LABEL_CLS}>Type</span>
              <div className="mt-1.5 inline-flex overflow-hidden rounded-lg border border-[rgba(255,135,175,0.15)]">
                {(['stdio', 'http'] as const).map((t) => (
                  <button
                    key={t}
                    type="button"
                    onClick={() => updateField('type', t)}
                    className={`flex items-center gap-1.5 px-4 py-2 text-[12px] font-medium transition-colors ${
                      form.type === t
                        ? 'bg-[rgba(255,135,175,0.12)] text-[var(--axon-accent-pink-strong)]'
                        : 'bg-[rgba(10,18,35,0.5)] text-[var(--axon-text-dim)] hover:bg-[rgba(10,18,35,0.7)] hover:text-[var(--axon-text-secondary)]'
                    }`}
                  >
                    {t === 'stdio' ? <Terminal className="size-3" /> : <Globe className="size-3" />}
                    {t}
                  </button>
                ))}
              </div>
            </div>

            {form.type === 'stdio' ? (
              <>
                <div>
                  <label htmlFor="mcp-command" className={LABEL_CLS}>
                    Command
                  </label>
                  <input
                    id="mcp-command"
                    type="text"
                    value={form.command}
                    onChange={(e) => updateField('command', e.target.value)}
                    placeholder="node"
                    className={`${INPUT_CLS} font-mono`}
                  />
                </div>
                <div>
                  <label htmlFor="mcp-args" className={LABEL_CLS}>
                    Args (one per line)
                  </label>
                  <textarea
                    id="mcp-args"
                    value={form.args}
                    onChange={(e) => updateField('args', e.target.value)}
                    placeholder={'/path/to/server.js\n--port\n3000'}
                    rows={3}
                    className={`${INPUT_CLS} resize-none font-mono leading-relaxed`}
                  />
                </div>
                <KvEditor
                  label="Environment variables"
                  pairs={form.envPairs}
                  onChange={(pairs) => updateField('envPairs', pairs)}
                />
              </>
            ) : (
              <>
                <div>
                  <label htmlFor="mcp-url" className={LABEL_CLS}>
                    URL
                  </label>
                  <input
                    id="mcp-url"
                    type="text"
                    value={form.url}
                    onChange={(e) => updateField('url', e.target.value)}
                    placeholder="https://example.com/mcp"
                    className={`${INPUT_CLS} font-mono`}
                  />
                </div>
                <KvEditor
                  label="Headers"
                  pairs={form.headerPairs}
                  onChange={(pairs) => updateField('headerPairs', pairs)}
                />
              </>
            )}

            <div className="flex justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={onCancel}
                className="rounded-lg px-3.5 py-2 text-[12px] font-medium text-[var(--axon-text-dim)] transition-colors hover:bg-[rgba(255,135,175,0.08)] hover:text-[var(--axon-text-secondary)]"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleSaveForm}
                disabled={!form.name.trim() || nameConflict}
                className="rounded-lg bg-[rgba(175,215,255,0.12)] px-4 py-2 text-[12px] font-semibold text-[var(--axon-accent-pink)] transition-colors hover:bg-[rgba(175,215,255,0.18)] disabled:cursor-not-allowed disabled:opacity-40"
              >
                Save
              </button>
            </div>
          </div>
        ) : (
          <div className="space-y-3">
            <p className="text-[11px] text-[var(--axon-text-dim)]">
              Edit the full mcp.json content directly. The first server entry will be saved.
            </p>
            <textarea
              value={rawJson}
              onChange={(e) => {
                jsonEditedManuallyRef.current = true
                setRawJson(e.target.value)
              }}
              rows={14}
              spellCheck={false}
              className="w-full resize-none rounded-lg border border-[rgba(255,135,175,0.15)] bg-[rgba(10,18,35,0.6)] px-3 py-2.5 font-mono text-[12px] leading-relaxed text-[var(--axon-text-secondary)] outline-none focus:border-[rgba(175,215,255,0.35)]"
            />
            {jsonError && <p className="text-[11px] text-red-400">{jsonError}</p>}
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={onCancel}
                className="rounded-lg px-3.5 py-2 text-[12px] font-medium text-[var(--axon-text-dim)] transition-colors hover:bg-[rgba(255,135,175,0.08)] hover:text-[var(--axon-text-secondary)]"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleSaveJson}
                className="rounded-lg bg-[rgba(175,215,255,0.12)] px-4 py-2 text-[12px] font-semibold text-[var(--axon-accent-pink)] transition-colors hover:bg-[rgba(175,215,255,0.18)]"
              >
                Save JSON
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
