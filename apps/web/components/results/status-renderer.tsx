'use client'

import type { DedupeResult, NormalizedResult, StatsResult } from '@/lib/result-types'
import { fmtNum } from './shared'

interface StatusRendererProps {
  result: NormalizedResult
}

export function StatusRenderer({ result }: StatusRendererProps) {
  switch (result.type) {
    case 'stats':
      return <StatsPanel data={result.data} />
    case 'dedupe':
      return <DedupePanel data={result.data} />
    default:
      return null
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function KvRow({ label, value, accent }: { label: string; value: string; accent?: string }) {
  return (
    <div className="flex justify-between py-1 text-[12px]">
      <span className="text-[var(--axon-text-muted)]">{label}</span>
      <span className={`tabular-nums ${accent ?? 'text-[var(--axon-accent-blue)]'}`}>{value}</span>
    </div>
  )
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="mb-1.5 mt-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--axon-text-dim)] first:mt-0">
      {children}
    </h3>
  )
}

// ---------------------------------------------------------------------------
// Stats panel
// ---------------------------------------------------------------------------

function StatsPanel({ data }: { data: StatsResult }) {
  // Separate known fields from extra/unknown fields
  const knownKeys = new Set([
    'collection',
    'status',
    'indexed_vectors_count',
    'points_count',
    'dimension',
    'distance',
    'segments_count',
    'docs_embedded_estimate',
    'avg_chunks_per_doc',
    'payload_fields',
    'counts',
  ])

  const extraFields = Object.entries(data).filter(
    ([k]) => !knownKeys.has(k) && typeof data[k] !== 'object',
  )

  const counts = data.counts ?? {}
  const countEntries = Object.entries(counts).sort(([, a], [, b]) => b - a)

  return (
    <div
      className="rounded-lg border border-[rgba(255,135,175,0.08)] p-4"
      style={{ background: 'rgba(10, 18, 35, 0.3)' }}
    >
      {/* Collection info */}
      <SectionHeader>Collection</SectionHeader>
      <div className="space-y-0.5">
        <KvRow label="Name" value={data.collection} />
        <KvRow
          label="Status"
          value={data.status}
          accent={
            data.status === 'green' ? 'text-[var(--axon-success)]' : 'text-[var(--axon-warning)]'
          }
        />
        <KvRow label="Distance" value={data.distance} />
        <KvRow label="Dimension" value={String(data.dimension)} />
        <KvRow label="Segments" value={fmtNum(data.segments_count)} />
      </div>

      {/* Vector counts */}
      <SectionHeader>Vectors</SectionHeader>
      <div className="space-y-0.5">
        <KvRow label="Points" value={fmtNum(data.points_count)} />
        <KvRow label="Indexed vectors" value={fmtNum(data.indexed_vectors_count)} />
        <KvRow label="Docs (estimate)" value={fmtNum(data.docs_embedded_estimate)} />
        <KvRow label="Avg chunks/doc" value={data.avg_chunks_per_doc.toFixed(1)} />
      </div>

      {/* Payload fields */}
      {data.payload_fields && data.payload_fields.length > 0 && (
        <>
          <SectionHeader>Payload Fields</SectionHeader>
          <div className="flex flex-wrap gap-1.5">
            {data.payload_fields.map((f) => (
              <span
                key={f}
                className="rounded-md border border-[rgba(255,135,175,0.1)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--axon-text-muted)]"
              >
                {f}
              </span>
            ))}
          </div>
        </>
      )}

      {/* Command usage counts */}
      {countEntries.length > 0 && (
        <>
          <SectionHeader>Command Usage</SectionHeader>
          <div className="grid grid-cols-2 gap-x-6 gap-y-0.5">
            {countEntries.map(([cmd, count]) => (
              <KvRow key={cmd} label={cmd} value={fmtNum(count)} />
            ))}
          </div>
        </>
      )}

      {/* Extra fields */}
      {extraFields.length > 0 && (
        <>
          <SectionHeader>Additional</SectionHeader>
          <div className="space-y-0.5">
            {extraFields.map(([key, val]) => (
              <KvRow key={key} label={key} value={String(val)} />
            ))}
          </div>
        </>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Dedupe panel
// ---------------------------------------------------------------------------

function DedupePanel({ data }: { data: DedupeResult }) {
  return (
    <div
      className="rounded-lg border border-[rgba(255,135,175,0.08)] p-4"
      style={{ background: 'rgba(10, 18, 35, 0.3)' }}
    >
      <SectionHeader>Deduplication Results</SectionHeader>
      <div className="space-y-0.5">
        <KvRow label="Collection" value={data.collection} />
        <KvRow label="Duplicate groups" value={fmtNum(data.duplicate_groups)} />
        <KvRow
          label="Points deleted"
          value={fmtNum(data.deleted)}
          accent={
            data.deleted > 0 ? 'text-[var(--axon-accent-pink)]' : 'text-[var(--axon-success)]'
          }
        />
      </div>
    </div>
  )
}
