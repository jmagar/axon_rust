'use client'

import { Globe, Terminal, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

import { KvEditor } from './kv-editor'
import type { FormState, McpConfig, McpServerConfig } from './mcp-types'
import { formToConfig, INPUT_CLS, LABEL_CLS } from './mcp-types'

// ── Re-exports (preserve public API for page.tsx) ─────────────────────────────

export { KvEditor } from './kv-editor'
export { McpServerCard } from './mcp-server-card'
export type {
  FormState,
  KvPair,
  McpConfig,
  McpServerConfig,
  McpServerStatus,
  ServerType,
} from './mcp-types'
export { configToForm, EMPTY_FORM, formToConfig, INPUT_CLS, LABEL_CLS } from './mcp-types'

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

  function handleSave() {
    if (activeTab === 'form') {
      handleSaveForm()
    } else {
      handleSaveJson()
    }
  }

  const nameConflict = !isEditing && existingNames.includes(form.name.trim())

  return (
    <div
      className="overflow-hidden rounded-xl border border-[rgba(175,215,255,0.18)] bg-[rgba(3,7,18,0.65)] shadow-[0_0_32px_rgba(175,215,255,0.04)]"
      style={{ backdropFilter: 'blur(20px) saturate(180%)' }}
    >
      <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-4 py-3">
        <h3 className="text-[13px] font-semibold text-[var(--text-primary)]">
          {isEditing ? `Edit — ${initial.name}` : 'Add MCP Server'}
        </h3>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-md p-1 text-[var(--text-dim)] hover:bg-[var(--surface-float)] hover:text-[var(--text-secondary)]"
          aria-label="Close"
        >
          <X className="size-4" />
        </button>
      </div>

      {/* Tab bar */}
      <div className="flex border-b border-[var(--border-subtle)]">
        {(['form', 'json'] as const).map((tab) => (
          <button
            key={tab}
            type="button"
            onClick={() => setActiveTab(tab)}
            className={`px-4 py-2 text-[12px] font-medium transition-colors ${
              activeTab === tab
                ? 'border-b-2 border-[var(--axon-primary-strong)] text-[var(--axon-primary-strong)]'
                : 'text-[var(--text-dim)] hover:text-[var(--text-secondary)]'
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
              <div className="mt-1.5 inline-flex overflow-hidden rounded-lg border border-[var(--border-subtle)]">
                {(['stdio', 'http'] as const).map((t) => (
                  <button
                    key={t}
                    type="button"
                    onClick={() => updateField('type', t)}
                    className={`flex items-center gap-1.5 px-4 py-2 text-[12px] font-medium transition-colors ${
                      form.type === t
                        ? 'bg-[rgba(175,215,255,0.12)] text-[var(--axon-primary)]'
                        : 'bg-[rgba(10,18,35,0.5)] text-[var(--text-dim)] hover:bg-[rgba(10,18,35,0.7)] hover:text-[var(--text-secondary)]'
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
          </div>
        ) : (
          <div className="space-y-3">
            <p className="text-[11px] text-[var(--text-dim)]">
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
              className="w-full resize-none rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.6)] px-3 py-2.5 font-mono text-[12px] leading-relaxed text-[var(--text-secondary)] outline-none focus:border-[var(--focus-ring-color)]"
            />
            {jsonError && <p className="text-[11px] text-red-400">{jsonError}</p>}
          </div>
        )}
      </div>

      {/* Sticky save footer */}
      <div className="sticky bottom-0 border-t border-[var(--border-subtle)] bg-[var(--surface-base)] p-3">
        <div className="flex items-center justify-between gap-3">
          {activeTab === 'json' && (
            <span className="text-xs text-[var(--text-dim)]">JSON reflects form values</span>
          )}
          <button
            type="button"
            onClick={handleSave}
            disabled={activeTab === 'form' && (!form.name.trim() || nameConflict)}
            className="ml-auto rounded-md bg-[rgba(135,175,255,0.15)] border border-[var(--border-standard)] px-4 py-1.5 text-xs font-medium text-[var(--axon-primary)] hover:bg-[rgba(135,175,255,0.25)] transition-all hover:scale-[1.02] disabled:cursor-not-allowed disabled:opacity-40"
          >
            {isEditing ? 'Save changes' : 'Add server'}
          </button>
        </div>
      </div>
    </div>
  )
}
