'use client'

export interface LogEntry {
  text: string
  ts: number
  service?: string
}

// Matches: 2026-03-01T07:06:25.417745Z  INFO axon::crates::core::logging: message
const TRACING_RE =
  /^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})\.\d+Z\s+(TRACE|DEBUG|INFO|WARN|ERROR)\s+([\w:]+):\s*(.+)$/

type Level = 'TRACE' | 'DEBUG' | 'INFO' | 'WARN' | 'ERROR'

// Design system mapping (docs/UI-DESIGN-SYSTEM.md §1):
//   Blue (--axon-primary) = informational/actionable
//   Pink (--axon-secondary) = accent/alert/error
//   --axon-warning = orange warning
//   --axon-success = green success
const LEVEL_TEXT: Record<Level, string> = {
  TRACE: 'text-[var(--text-dim)]',
  DEBUG: 'text-[var(--axon-primary)]', // blue = informational
  INFO: 'text-[var(--axon-success)]', // green = success/info
  WARN: 'text-[var(--axon-warning)]', // orange = warning
  ERROR: 'text-[var(--axon-secondary-strong)]', // pink = error/alert
}

// Semi-transparent tinted backgrounds derived from design system palette.
// Success-bg and warning-bg have tokens; blue and pink don't, so we use
// the same rgba pattern as --axon-success-bg / --axon-warning-bg.
const LEVEL_BG: Record<Level, string> = {
  TRACE: '',
  DEBUG: 'bg-[rgba(135,175,255,0.10)]', // --axon-primary tint
  INFO: 'bg-[var(--axon-success-bg)]', // rgba(130,217,160,0.14)
  WARN: 'bg-[var(--axon-warning-bg)]', // rgba(255,192,134,0.14)
  ERROR: 'bg-[rgba(255,135,175,0.12)]', // --axon-secondary tint
}

const MESSAGE_TEXT: Record<Level, string> = {
  TRACE: 'text-[var(--text-dim)]',
  DEBUG: 'text-[var(--text-secondary)]',
  INFO: 'text-[var(--text-primary)]',
  WARN: 'text-[var(--axon-warning)]',
  ERROR: 'text-[var(--axon-secondary)]',
}

// One distinct color per service — cycles through design system palette
const SERVICE_COLORS: Record<string, string> = {
  'axon-workers': 'var(--axon-primary)', // blue
  'axon-web': 'var(--axon-secondary)', // pink
  'axon-postgres': 'var(--axon-success)', // green
  'axon-redis': 'var(--axon-warning)', // orange
  'axon-rabbitmq': 'var(--axon-primary-strong)', // brighter blue
  'axon-qdrant': 'rgba(174,136,255,0.9)', // purple
  'axon-chrome': 'rgba(255,220,100,0.9)', // yellow
}

// axon::crates::core::logging → core::logging (last 2 segments)
function shortenModule(mod: string): string {
  const parts = mod.split('::')
  return parts.length > 2 ? parts.slice(-2).join('::') : mod
}

function parseLine(text: string) {
  const m = TRACING_RE.exec(text)
  if (!m) return null
  const [, datetime, levelRaw, module, message] = m
  return {
    time: datetime.split('T')[1], // HH:MM:SS
    level: levelRaw as Level,
    module: shortenModule(module),
    message,
  }
}

// Unstructured lines: colorize by keyword using design tokens, not raw Tailwind colors
function fallbackColor(text: string): string {
  const l = text.toLowerCase()
  if (l.includes('error')) return 'text-[var(--axon-secondary)]'
  if (l.includes('warn')) return 'text-[var(--axon-warning)]'
  if (l.includes('debug')) return 'text-[var(--axon-primary)]'
  return 'text-[var(--text-secondary)]'
}

interface LogLineProps {
  entry: LogEntry
}

function ServiceBadge({ service }: { service: string }) {
  const color = SERVICE_COLORS[service] ?? 'var(--text-dim)'
  const label = service.replace('axon-', '')
  return (
    <span
      className="shrink-0 font-mono text-[length:var(--text-2xs)] font-semibold uppercase tabular-nums"
      style={{ color, minWidth: '4.5rem' }}
    >
      {label}
    </span>
  )
}

export function LogLine({ entry }: LogLineProps) {
  const parsed = parseLine(entry.text)

  if (parsed) {
    const { time, level, module, message } = parsed
    return (
      <div
        className="flex min-w-0 select-text items-baseline gap-2 break-all"
        style={{ paddingBlock: '2px', lineHeight: 'var(--leading-copy)' }}
      >
        {entry.service && <ServiceBadge service={entry.service} />}

        {/* Timestamp — text-2xs (10px) per design system chip/micro-label scale */}
        <span className="shrink-0 font-mono text-[length:var(--text-2xs)] tabular-nums text-[var(--text-dim)]">
          {time}
        </span>

        {/* Level badge — ui-chip-status provides pill shape, 10px, 600w, uppercase */}
        <span className={`ui-chip-status shrink-0 ${LEVEL_TEXT[level]} ${LEVEL_BG[level]}`}>
          {level}
        </span>

        {/* Module path — ui-mono: text-sm (12px) */}
        <span className="ui-mono shrink-0 text-[var(--text-dim)]">{module}</span>

        {/* Message — ui-mono for code-adjacent content */}
        <span className={`ui-mono min-w-0 ${MESSAGE_TEXT[level]}`}>{message}</span>
      </div>
    )
  }

  // Unstructured line (docker system messages, pnpm output, etc.)
  return (
    <div
      className={`flex min-w-0 select-text items-baseline gap-2 break-all ${fallbackColor(entry.text)}`}
      style={{ paddingBlock: '2px', lineHeight: 'var(--leading-copy)' }}
    >
      {entry.service && <ServiceBadge service={entry.service} />}
      <span className="ui-mono min-w-0">{entry.text}</span>
    </div>
  )
}
