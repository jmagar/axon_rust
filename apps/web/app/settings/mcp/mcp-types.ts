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
  'w-full rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.5)] px-3 py-2.5 text-[13px] text-[var(--text-secondary)] outline-none placeholder:text-[var(--text-dim)] focus:border-[var(--focus-ring-color)] focus:bg-[rgba(10,18,35,0.7)]'

export const LABEL_CLS =
  'mb-1.5 block text-[11px] font-medium uppercase tracking-[0.07em] text-[var(--text-dim)]'

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

export type McpServerStatus = 'online' | 'offline' | 'unknown' | 'checking'
