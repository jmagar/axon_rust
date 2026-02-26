'use client'

import type { PulsePermissionLevel } from '@/lib/pulse/types'

interface PulseToolbarProps {
  title: string
  onTitleChange: (title: string) => void
  permissionLevel: PulsePermissionLevel
  onPermissionChange: (level: PulsePermissionLevel) => void
  saveStatus: 'idle' | 'saving' | 'saved' | 'error'
}

const PERMISSION_OPTIONS: { value: PulsePermissionLevel; label: string }[] = [
  { value: 'plan', label: 'Plan' },
  { value: 'training-wheels', label: 'Training Wheels' },
  { value: 'full-access', label: 'Full Access' },
]

export function PulseToolbar({
  title,
  onTitleChange,
  permissionLevel,
  onPermissionChange,
  saveStatus,
}: PulseToolbarProps) {
  return (
    <div className="flex items-center justify-between rounded-lg border border-[rgba(255,135,175,0.08)] bg-[rgba(10,18,35,0.3)] px-3 py-1.5">
      <input
        value={title}
        onChange={(e) => onTitleChange(e.target.value)}
        className="bg-transparent text-sm font-medium text-[var(--axon-text-primary)] outline-none placeholder:text-[var(--axon-text-subtle)]"
        placeholder="Document title..."
      />

      <div className="flex items-center gap-3">
        <span className="text-[10px] text-[var(--axon-text-dim)]">
          {saveStatus === 'saving' && 'Saving...'}
          {saveStatus === 'saved' && 'Saved'}
          {saveStatus === 'error' && 'Save failed'}
        </span>

        <div className="flex items-center gap-1">
          {PERMISSION_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => onPermissionChange(opt.value)}
              className={`rounded-md px-2 py-1 text-[10px] font-semibold uppercase tracking-wider transition-colors ${
                permissionLevel === opt.value
                  ? 'bg-[rgba(175,215,255,0.15)] text-[var(--axon-accent-pink)]'
                  : 'text-[var(--axon-text-dim)] hover:text-[var(--axon-text-muted)]'
              }`}
              title={opt.label}
            >
              {opt.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
