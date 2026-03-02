'use client'

import { Plus, X } from 'lucide-react'

import type { KvPair } from './mcp-types'
import { LABEL_CLS } from './mcp-types'

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
          className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] text-[var(--text-dim)] hover:bg-[rgba(175,215,255,0.08)] hover:text-[var(--axon-primary-strong)]"
        >
          <Plus className="size-3" />
          Add
        </button>
      </div>
      {pairs.length === 0 ? (
        <p className="text-[11px] text-[var(--text-dim)]">None configured.</p>
      ) : (
        <div className="space-y-2">
          {pairs.map((p, i) => (
            <div key={`kv-${p.id}`} className="flex items-center gap-2">
              <input
                type="text"
                value={p.key}
                onChange={(e) => updatePair(i, 'key', e.target.value)}
                placeholder="KEY"
                className="w-2/5 rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.5)] px-2.5 py-2 font-mono text-[12px] text-[var(--text-secondary)] outline-none placeholder:text-[var(--text-dim)] focus:border-[var(--focus-ring-color)]"
              />
              <input
                type="text"
                value={p.value}
                onChange={(e) => updatePair(i, 'value', e.target.value)}
                placeholder="value"
                className="min-w-0 flex-1 rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.5)] px-2.5 py-2 text-[12px] text-[var(--text-secondary)] outline-none placeholder:text-[var(--text-dim)] focus:border-[var(--focus-ring-color)]"
              />
              <button
                type="button"
                onClick={() => removePair(i)}
                className="shrink-0 rounded p-1 text-[var(--text-dim)] hover:bg-[rgba(255,100,100,0.12)] hover:text-red-400"
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
