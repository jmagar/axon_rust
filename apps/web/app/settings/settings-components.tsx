'use client'

export function SectionHeader({
  icon: Icon,
  label,
  description,
}: {
  icon: React.ComponentType<{ className?: string }>
  label: string
  description?: string
}) {
  return (
    <div className="mb-5">
      <div className="flex items-center gap-3">
        <div className="flex size-8 shrink-0 items-center justify-center rounded-lg border border-[var(--border-accent)] bg-[rgba(255,135,175,0.08)] shadow-[var(--shadow-sm)]">
          <Icon className="size-4 text-[var(--axon-secondary)]" />
        </div>
        <h2 className="font-display text-base font-bold text-[var(--text-primary)]">{label}</h2>
      </div>
      {description && (
        <p className="mt-1.5 ml-11 text-sm leading-relaxed text-[var(--text-muted)]">
          {description}
        </p>
      )}
    </div>
  )
}

export function FieldHint({ children }: { children: React.ReactNode }) {
  return <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--text-dim)]">{children}</p>
}

export function SectionDivider() {
  return (
    <div className="my-8 h-px bg-gradient-to-r from-transparent via-[var(--border-subtle)] to-transparent" />
  )
}

export function ToggleRow({
  id,
  label,
  description,
  cliFlag,
  checked,
  onChange,
}: {
  id: string
  label: string
  description: string
  cliFlag?: string
  checked: boolean
  onChange: (v: boolean) => void
}) {
  return (
    <div
      className="flex items-start justify-between gap-4 rounded-xl border border-[var(--border-subtle)] px-4 py-3.5 transition-all duration-200"
      style={{ background: 'rgba(10,18,35,0.58)', backdropFilter: 'blur(8px)' }}
    >
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-medium text-[var(--text-secondary)]">{label}</p>
        <p className="mt-0.5 text-[11px] text-[var(--text-dim)]">
          {description}
          {cliFlag && (
            <>
              {' '}
              <code className="rounded bg-[rgba(175,215,255,0.07)] px-1 py-0.5 font-mono text-[10px] text-[var(--text-muted)]">
                {cliFlag}
              </code>
            </>
          )}
        </p>
      </div>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className="relative mt-0.5 inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-all duration-200 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgba(175,215,255,0.5)]"
        style={{ background: checked ? 'var(--axon-primary-strong)' : 'var(--surface-elevated)' }}
        aria-label={label}
      >
        <span
          className="inline-block size-3.5 rounded-full bg-white shadow-sm transition-transform duration-200"
          style={{ transform: checked ? 'translateX(18px)' : 'translateX(2px)' }}
        />
      </button>
    </div>
  )
}

export function TextInput({
  id,
  value,
  onChange,
  placeholder,
  mono,
}: {
  id: string
  value: string
  onChange: (v: string) => void
  placeholder?: string
  mono?: boolean
}) {
  return (
    <input
      id={id}
      type="text"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      className={`w-full rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.65)] px-3 py-2.5 text-[13px] text-[var(--text-secondary)] outline-none placeholder:text-[var(--text-dim)] focus:border-[var(--focus-ring-color)] focus:bg-[rgba(10,18,35,0.82)] transition-all duration-200 ${mono ? 'font-mono' : ''}`}
      style={{ backdropFilter: 'blur(4px)' }}
    />
  )
}

export const GLASS_SELECT =
  'w-full rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.65)] px-3 py-2.5 text-[13px] text-[var(--text-secondary)] outline-none focus:border-[var(--focus-ring-color)] focus:bg-[rgba(10,18,35,0.82)] cursor-pointer appearance-none transition-all duration-200'
