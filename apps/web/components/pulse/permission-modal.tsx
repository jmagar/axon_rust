'use client'

import { Check, ShieldAlert, ShieldCheck, ShieldOff, X } from 'lucide-react'
import { useCallback, useEffect, useRef } from 'react'
import { createPortal } from 'react-dom'

import type { AcpPermissionRequest } from '@/lib/pulse/types'

/** Human-readable labels and descriptions for each ACP permission option. */
const OPTION_META: Record<
  string,
  { label: string; description: string; icon: typeof Check; variant: 'allow' | 'reject' }
> = {
  'option-allow-once': {
    label: 'Allow Once',
    description: 'Approve this single invocation',
    icon: Check,
    variant: 'allow',
  },
  'option-allow-always': {
    label: 'Allow Always',
    description: 'Approve all future invocations of this tool',
    icon: ShieldCheck,
    variant: 'allow',
  },
  'option-reject-once': {
    label: 'Reject Once',
    description: 'Deny this invocation',
    icon: X,
    variant: 'reject',
  },
  'option-reject-always': {
    label: 'Reject Always',
    description: 'Deny all future invocations of this tool',
    icon: ShieldOff,
    variant: 'reject',
  },
}

interface PermissionModalProps {
  request: AcpPermissionRequest
  /** When true, the modal is informational-only and auto-dismisses after a short delay. */
  autoApprove: boolean
  onRespond: (toolCallId: string, optionId: string) => void
  onDismiss: () => void
}

export function PermissionModal({
  request,
  autoApprove,
  onRespond,
  onDismiss,
}: PermissionModalProps) {
  const autoApproveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const handleSelect = useCallback(
    (optionId: string) => {
      onRespond(request.toolCallId, optionId)
    },
    [onRespond, request.toolCallId],
  )

  // Auto-approve mode: select 'option-allow-once' after 2s, then dismiss
  useEffect(() => {
    if (!autoApprove) return
    autoApproveTimerRef.current = setTimeout(() => {
      const allowOnce = request.options.find((o) => o === 'option-allow-once')
      if (allowOnce) {
        handleSelect(allowOnce)
      } else {
        onDismiss()
      }
    }, 2000)
    return () => {
      if (autoApproveTimerRef.current) clearTimeout(autoApproveTimerRef.current)
    }
  }, [autoApprove, handleSelect, onDismiss, request.options])

  // Keyboard: Escape to dismiss (reject-once in blocking mode, dismiss in auto mode)
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || (e.target as HTMLElement)?.isContentEditable) {
        return
      }
      if (e.key === 'Escape') {
        if (autoApprove) {
          onDismiss()
        } else {
          const rejectOnce = request.options.find((o) => o === 'option-reject-once')
          if (rejectOnce) handleSelect(rejectOnce)
          else onDismiss()
        }
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [autoApprove, handleSelect, onDismiss, request.options])

  const toolName = request.toolName ?? request.toolCallId

  return createPortal(
    <div
      className="fixed inset-0 z-[9999] flex items-center justify-center bg-black/40"
      onClick={(e) => {
        if (e.target === e.currentTarget) {
          if (autoApprove) onDismiss()
        }
      }}
      onKeyDown={undefined}
      role="dialog"
      aria-modal="true"
      aria-label="Tool permission request"
    >
      <div className="mx-4 w-full max-w-md rounded-xl border border-[rgba(175,215,255,0.3)] bg-[var(--bg-primary,#0a1223)] p-6 shadow-2xl shadow-black/40">
        {/* Header */}
        <div className="mb-4 flex items-center gap-2.5">
          <ShieldAlert className="size-5 shrink-0 text-[var(--axon-primary-strong)]" />
          <h4 className="text-sm font-bold uppercase tracking-wider text-[var(--axon-primary-strong)]">
            Permission Request
          </h4>
          {autoApprove && (
            <span className="ml-auto rounded bg-[rgba(175,215,255,0.12)] px-2 py-0.5 text-[11px] font-medium text-[var(--text-dim)]">
              AUTO-APPROVE
            </span>
          )}
        </div>

        {/* Tool info */}
        <div className="mb-4 rounded-lg border border-[var(--border-subtle)] bg-[rgba(10,18,35,0.6)] px-3 py-2.5">
          <p className="text-xs text-[var(--text-dim)]">Tool requesting access:</p>
          <p className="mt-1 font-mono text-sm font-semibold text-[var(--text-primary)]">
            {toolName}
          </p>
        </div>

        {autoApprove ? (
          <p className="mb-4 text-sm text-[var(--text-muted)]">
            Auto-approving in 2 seconds. This tool invocation will be allowed automatically.
          </p>
        ) : (
          <p className="mb-4 text-sm text-[var(--text-muted)]">
            The agent wants to use this tool. Choose how to respond:
          </p>
        )}

        {/* Permission buttons */}
        <div className="flex flex-col gap-2">
          {request.options.map((optionId) => {
            const meta = OPTION_META[optionId]
            if (!meta) {
              return (
                <button
                  key={optionId}
                  type="button"
                  onClick={() => handleSelect(optionId)}
                  className="rounded-md bg-[rgba(175,215,255,0.1)] px-4 py-2 text-left text-sm text-[var(--text-secondary)] transition-colors hover:bg-[rgba(175,215,255,0.2)]"
                >
                  {optionId}
                </button>
              )
            }
            const Icon = meta.icon
            const isAllow = meta.variant === 'allow'
            return (
              <button
                key={optionId}
                type="button"
                onClick={() => handleSelect(optionId)}
                disabled={autoApprove}
                className={`group/btn flex items-center gap-3 rounded-md px-4 py-2.5 text-left transition-colors disabled:pointer-events-none disabled:opacity-50 ${
                  isAllow
                    ? 'bg-[rgba(175,215,255,0.08)] text-[var(--axon-primary-strong)] hover:bg-[rgba(175,215,255,0.2)]'
                    : 'bg-[rgba(255,135,175,0.06)] text-[var(--text-muted)] hover:bg-[rgba(255,135,175,0.14)] hover:text-[var(--axon-secondary)]'
                }`}
              >
                <Icon
                  className={`size-4 shrink-0 ${
                    isAllow
                      ? 'text-[var(--axon-primary)]'
                      : 'text-[var(--text-dim)] group-hover/btn:text-[var(--axon-secondary)]'
                  }`}
                />
                <div className="min-w-0 flex-1">
                  <span className="block text-sm font-semibold">{meta.label}</span>
                  <span className="block text-[11px] text-[var(--text-dim)]">
                    {meta.description}
                  </span>
                </div>
              </button>
            )
          })}
        </div>

        <p className="mt-3 text-[11px] text-[var(--text-dim)]">
          {autoApprove
            ? 'Disable auto-approve in Settings to review each request'
            : 'Press Esc to reject once'}
        </p>
      </div>
    </div>,
    document.body,
  )
}
