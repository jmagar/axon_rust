'use client'

import { useCallback, useState } from 'react'
import { AXON_COMMAND_OPTIONS, type AxonOptionSpec, getCommandSpec } from '@/lib/axon-command-map'

export interface CommandOptionValues {
  [key: string]: string | boolean | number
}

interface CommandOptionsPanelProps {
  mode: string
  values: CommandOptionValues
  onChange: (values: CommandOptionValues) => void
}

function parseEnumValues(notes?: string): string[] {
  if (!notes) return []
  // Match patterns like "hot|top|new|rising" or "hour|day|week|month|year|all"
  const pipeMatch = notes.match(/:\s*([a-zA-Z0-9_|]+)$/)
  if (pipeMatch) {
    return pipeMatch[1].split('|').filter(Boolean)
  }
  return []
}

function getOptionSpec(key: string): AxonOptionSpec | undefined {
  return AXON_COMMAND_OPTIONS.find((o) => o.key === key)
}

function OptionControl({
  optionKey,
  spec,
  value,
  onUpdate,
}: {
  optionKey: string
  spec: AxonOptionSpec
  value: string | boolean | number | undefined
  onUpdate: (key: string, val: string | boolean | number) => void
}) {
  const label = optionKey.replace(/_/g, ' ')

  switch (spec.value) {
    case 'bool':
      return (
        <label className="flex cursor-pointer items-center gap-2.5 rounded-lg px-3 py-2 transition-colors hover:bg-[var(--surface-float)]">
          <button
            type="button"
            role="checkbox"
            aria-checked={!!value}
            onClick={() => onUpdate(optionKey, !value)}
            className={`flex size-4 shrink-0 items-center justify-center rounded border transition-all focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] ${
              value
                ? 'border-[var(--axon-secondary)] bg-[rgba(255,135,175,0.18)]'
                : 'border-[var(--border-accent)] bg-transparent hover:border-[var(--border-strong)]'
            }`}
          >
            {value && (
              <svg
                className="size-3 text-[var(--axon-primary-strong)]"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={3}
              >
                <path d="M5 12l5 5L20 7" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            )}
          </button>
          <span className="text-xs text-[var(--text-muted)]">{label}</span>
        </label>
      )

    case 'number':
      return (
        <label className="flex items-center gap-2.5 rounded-lg px-3 py-2">
          <span className="shrink-0 text-xs text-[var(--text-muted)]">{label}</span>
          <input
            type="number"
            value={value !== undefined ? String(value) : ''}
            onChange={(e) => {
              const n = Number.parseInt(e.target.value, 10)
              if (!Number.isNaN(n)) onUpdate(optionKey, n)
            }}
            placeholder="—"
            className="w-20 rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.5)] px-2 py-1 font-mono text-xs text-[var(--axon-secondary)] outline-none placeholder:text-[var(--text-dim)] focus:border-[var(--focus-ring-color)]"
          />
        </label>
      )

    case 'enum': {
      const options = parseEnumValues(spec.notes)
      return (
        <label className="flex items-center gap-2.5 rounded-lg px-3 py-2">
          <span className="shrink-0 text-xs text-[var(--text-muted)]">{label}</span>
          <select
            value={value !== undefined ? String(value) : ''}
            onChange={(e) => onUpdate(optionKey, e.target.value)}
            className="rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.5)] px-2 py-1 font-mono text-xs text-[var(--axon-secondary)] outline-none focus:border-[var(--focus-ring-color)]"
          >
            <option value="">default</option>
            {options.map((opt) => (
              <option key={opt} value={opt}>
                {opt}
              </option>
            ))}
          </select>
        </label>
      )
    }

    case 'string':
    case 'list':
      return (
        <label className="flex items-center gap-2.5 rounded-lg px-3 py-2">
          <span className="shrink-0 text-xs text-[var(--text-muted)]">{label}</span>
          <input
            type="text"
            value={value !== undefined ? String(value) : ''}
            onChange={(e) => onUpdate(optionKey, e.target.value)}
            placeholder={spec.value === 'list' ? 'comma-separated' : '—'}
            className="w-40 rounded border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.5)] px-2 py-1 font-mono text-xs text-[var(--axon-secondary)] outline-none placeholder:text-[var(--text-dim)] focus:border-[var(--focus-ring-color)]"
          />
        </label>
      )

    default:
      return null
  }
}

export function CommandOptionsPanel({ mode, values, onChange }: CommandOptionsPanelProps) {
  const [expanded, setExpanded] = useState(false)

  // useCallback must be called unconditionally (Rules of Hooks)
  const handleUpdate = useCallback(
    (key: string, val: string | boolean | number) => {
      onChange({ ...values, [key]: val })
    },
    [values, onChange],
  )

  const spec = getCommandSpec(mode)
  const optionKeys = spec?.commandOptions ?? []
  if (optionKeys.length === 0) return null

  const resolvedOptions = optionKeys
    .map((key) => ({ key, spec: getOptionSpec(key) }))
    .filter((o): o is { key: string; spec: AxonOptionSpec } => o.spec !== undefined)

  if (resolvedOptions.length === 0) return null

  return (
    <div
      className="overflow-hidden rounded-lg border border-[var(--border-subtle)] transition-all duration-200"
      style={{ background: 'rgba(10, 18, 35, 0.45)' }}
    >
      <button
        type="button"
        onClick={() => setExpanded((prev) => !prev)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left transition-colors hover:bg-[var(--surface-float)]"
      >
        <svg
          className={`size-3 shrink-0 text-[var(--text-dim)] transition-transform duration-200 ${expanded ? 'rotate-90' : ''}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
        </svg>
        <span className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
          Options
        </span>
        <span className="text-[10px] text-[var(--text-dim)]">({resolvedOptions.length})</span>
      </button>

      {expanded && (
        <div className="flex flex-wrap gap-x-2 gap-y-0.5 border-t border-[var(--border-subtle)] px-1 pb-2 pt-1">
          {resolvedOptions.map(({ key, spec: optSpec }) => (
            <OptionControl
              key={key}
              optionKey={key}
              spec={optSpec}
              value={values[key]}
              onUpdate={handleUpdate}
            />
          ))}
        </div>
      )}
    </div>
  )
}
