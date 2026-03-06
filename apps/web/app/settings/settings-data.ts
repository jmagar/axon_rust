import {
  Bot,
  Brain,
  Cpu,
  Gauge,
  Server,
  Shield,
  ShieldCheck,
  ShieldOff,
  Sparkles,
  Terminal,
  Wrench,
  Zap,
} from 'lucide-react'
import type { PulseModel, PulsePermissionLevel } from '@/lib/pulse/types'

export const CLAUDE_MODEL_IDS = ['sonnet', 'opus', 'haiku'] as const

export const CLAUDE_MODEL_OPTIONS: {
  id: PulseModel
  label: string
  sub: string
  badge?: string
}[] = [
  {
    id: 'sonnet',
    label: 'Claude Sonnet 4.6',
    sub: 'Balanced intelligence and speed',
    badge: 'Default',
  },
  { id: 'opus', label: 'Claude Opus 4.6', sub: 'Most capable — best for complex tasks' },
  { id: 'haiku', label: 'Claude Haiku 4.5', sub: 'Fastest response — most efficient' },
]

export const PERMISSION_OPTIONS: {
  id: PulsePermissionLevel
  label: string
  sub: string
  icon: React.ComponentType<{ className?: string }>
  accentColor: string
}[] = [
  {
    id: 'plan',
    label: 'Plan',
    sub: 'Read-only analysis — no file changes or commands executed',
    icon: Shield,
    accentColor: 'rgba(175,215,255,0.7)',
  },
  {
    id: 'accept-edits',
    label: 'Accept Edits',
    sub: 'Claude proposes changes; you confirm each edit before it applies',
    icon: ShieldCheck,
    accentColor: 'rgba(255,135,175,0.7)',
  },
  {
    id: 'bypass-permissions',
    label: 'Bypass Permissions',
    sub: 'Apply all changes directly without confirmation prompts',
    icon: ShieldOff,
    accentColor: 'rgba(255,175,100,0.7)',
  },
]

export const EFFORT_OPTIONS: {
  id: 'low' | 'medium' | 'high'
  label: string
  hint: string
  sub: string
}[] = [
  { id: 'low', label: 'Low', hint: 'Fastest', sub: 'Quick answers, minimal reasoning' },
  { id: 'medium', label: 'Medium', hint: 'Balanced', sub: 'Default thinking budget' },
  { id: 'high', label: 'High', hint: 'Thorough', sub: 'Extended reasoning, deepest analysis' },
]

export const FALLBACK_MODEL_OPTIONS: { value: string; label: string }[] = [
  { value: '', label: 'Disabled (no fallback)' },
  { value: 'sonnet', label: 'Sonnet' },
  { value: 'opus', label: 'Opus' },
  { value: 'haiku', label: 'Haiku' },
]

export const KEYBOARD_SHORTCUTS = [
  { keys: ['/', 'Ctrl+K'], desc: 'Focus the omnibox' },
  { keys: ['Alt', '1'], desc: 'Switch to Sonnet' },
  { keys: ['Alt', '2'], desc: 'Switch to Opus' },
  { keys: ['Alt', '3'], desc: 'Switch to Haiku' },
  { keys: ['Alt', 'Shift', '1'], desc: 'Plan permission mode' },
  { keys: ['Alt', 'Shift', '2'], desc: 'Accept Edits mode' },
  { keys: ['Alt', 'Shift', '3'], desc: 'Bypass Permissions mode' },
]

export const NAV_SECTIONS = [
  { id: 'model', label: 'Model', icon: Cpu },
  { id: 'permission', label: 'Permission Mode', icon: Shield },
  { id: 'effort', label: 'Reasoning Effort', icon: Brain },
  { id: 'limits', label: 'Limits', icon: Gauge },
  { id: 'instructions', label: 'Custom Instructions', icon: Sparkles },
  { id: 'tools', label: 'Tools & Permissions', icon: Wrench },
  { id: 'session', label: 'Session Behavior', icon: Terminal },
  { id: 'shortcuts', label: 'Keyboard Shortcuts', icon: Zap },
]

export { Bot, Server }
