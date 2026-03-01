'use client'

import type { ReactNode } from 'react'

interface StructuredDataViewProps {
  data: unknown
}

type Scalar = string | number | boolean | null

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isScalar(value: unknown): value is Scalar {
  return (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  )
}

function humanizeKey(key: string): string {
  return key
    .replace(/_/g, ' ')
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/\s+/g, ' ')
    .trim()
}

function scalarLabel(value: Scalar): string {
  if (value === null) return 'none'
  if (typeof value === 'boolean') return value ? 'yes' : 'no'
  return String(value)
}

function detectScalarRecordArray(value: unknown): Array<Record<string, Scalar>> | null {
  if (!Array.isArray(value) || value.length === 0) return null
  if (!value.every(isRecord)) return null

  const keys = new Set<string>()
  for (const row of value) {
    for (const [k, v] of Object.entries(row)) {
      if (isScalar(v)) keys.add(k)
    }
  }
  if (keys.size === 0) return null

  const keyList = Array.from(keys)
  if (keyList.length > 12) return null

  return value.map((row) => {
    const out: Record<string, Scalar> = {}
    for (const key of keyList) {
      const cell = row[key]
      out[key] = isScalar(cell) ? cell : null
    }
    return out
  })
}

function Section({
  title,
  children,
  muted = false,
}: {
  title?: string
  children: ReactNode
  muted?: boolean
}) {
  return (
    <div
      className="rounded-lg border p-3"
      style={{
        background: muted ? 'rgba(10, 18, 35, 0.25)' : 'rgba(10, 18, 35, 0.4)',
        borderColor: 'var(--border-subtle)',
      }}
    >
      {title && <div className="ui-label mb-2">{title}</div>}
      {children}
    </div>
  )
}

function KeyValueTable({ rows }: { rows: Array<{ key: string; value: Scalar }> }) {
  return (
    <div className="overflow-auto">
      <table className="ui-table-dense">
        <tbody>
          {rows.map((row) => (
            <tr key={row.key} className="border-b border-[var(--border-subtle)] last:border-b-0">
              <td className="ui-table-cell ui-table-cell-muted pr-4 align-top">
                {humanizeKey(row.key)}
              </td>
              <td className="ui-table-cell align-top">{scalarLabel(row.value)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function ScalarArrayTable({ rows }: { rows: Array<Record<string, Scalar>> }) {
  const columns = rows.length > 0 ? Object.keys(rows[0]) : []

  return (
    <div className="max-h-[55vh] overflow-auto">
      <table className="ui-table-dense">
        <thead className="sticky top-0" style={{ background: 'rgba(3, 7, 18, 0.95)' }}>
          <tr>
            {columns.map((col) => (
              <th key={col} className="ui-table-head pr-4">
                {humanizeKey(col)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, idx) => (
            <tr key={idx} className="border-b border-[var(--border-subtle)] last:border-b-0">
              {columns.map((col) => (
                <td key={col} className="ui-table-cell pr-4">
                  {scalarLabel(row[col])}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function NodeView({ value, title }: { value: unknown; title?: string }) {
  if (isScalar(value)) {
    return (
      <Section title={title}>
        <div className="ui-long-copy">{scalarLabel(value)}</div>
      </Section>
    )
  }

  if (Array.isArray(value)) {
    const tableRows = detectScalarRecordArray(value)
    if (tableRows) {
      return (
        <Section title={title}>
          <ScalarArrayTable rows={tableRows} />
        </Section>
      )
    }

    if (value.length === 0) {
      return (
        <Section title={title}>
          <div className="ui-meta">(empty list)</div>
        </Section>
      )
    }

    return (
      <Section title={title}>
        <div className="space-y-2">
          {value.map((item, idx) => (
            <NodeView key={idx} value={item} title={`Item ${idx + 1}`} />
          ))}
        </div>
      </Section>
    )
  }

  if (isRecord(value)) {
    const entries = Object.entries(value).filter(([, v]) => v !== undefined)
    const scalarRows = entries.filter(([, v]) => isScalar(v)) as Array<[string, Scalar]>
    const nestedRows = entries.filter(([, v]) => !isScalar(v))

    return (
      <Section title={title}>
        <div className="space-y-2">
          {scalarRows.length > 0 && (
            <KeyValueTable rows={scalarRows.map(([key, val]) => ({ key, value: val }))} />
          )}
          {nestedRows.map(([key, nested]) => (
            <NodeView key={key} value={nested} title={humanizeKey(key)} />
          ))}
          {scalarRows.length === 0 && nestedRows.length === 0 && (
            <div className="ui-meta">(empty object)</div>
          )}
        </div>
      </Section>
    )
  }

  return (
    <Section title={title}>
      <div className="ui-long-copy">{String(value)}</div>
    </Section>
  )
}

export function StructuredDataView({ data }: StructuredDataViewProps) {
  return <NodeView value={data} />
}
