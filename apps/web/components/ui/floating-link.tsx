'use client'

import {
  FloatingLinkNewTabInput,
  FloatingLinkUrlInput,
  submitFloatingLink,
  useFloatingLinkEdit,
  useFloatingLinkEditState,
  useFloatingLinkInsert,
  useFloatingLinkInsertState,
} from '@platejs/link/react'
import { ExternalLink, Link2Off } from 'lucide-react'
import { useEditorRef } from 'platejs/react'
import { cn } from '@/lib/utils'

const panelCls = cn(
  'flex flex-col gap-1.5 rounded-md border border-[var(--border-standard)]',
  'bg-[rgba(3,7,18,0.92)] p-2 shadow-2xl backdrop-blur-md',
  'min-w-[260px]',
)

const inputCls = cn(
  'w-full rounded border border-[var(--border-subtle)] bg-[rgba(255,255,255,0.04)]',
  'px-2 py-1 text-[11px] text-[var(--text-primary)] outline-none',
  'placeholder:text-[var(--text-muted)]',
  'focus:border-[var(--axon-secondary)] focus:ring-0',
)

const btnCls = cn(
  'rounded px-2 py-1 text-[10px] font-medium transition-colors',
  'bg-[rgba(255,135,175,0.12)] text-[var(--axon-secondary)]',
  'hover:bg-[rgba(255,135,175,0.22)]',
)

const iconBtnCls = cn(
  'flex items-center justify-center rounded p-1 transition-colors',
  'text-[var(--text-muted)] hover:bg-[rgba(255,255,255,0.06)] hover:text-[var(--text-primary)]',
)

/**
 * FloatingLink — URL input popover for inserting and editing links.
 *
 * Renders in two modes:
 * - Insert: triggered by Link toolbar button or Ctrl+K
 * - Edit: appears automatically when cursor is inside an existing link node
 *
 * Must be rendered inside a <Plate> context (e.g. inside <EditorContainer>).
 */
export function FloatingLink() {
  const editor = useEditorRef()

  const insertState = useFloatingLinkInsertState()
  const { props: insertFloatingProps } = useFloatingLinkInsert(insertState)

  const editState = useFloatingLinkEditState()
  const {
    editButtonProps,
    props: editFloatingProps,
    unlinkButtonProps,
  } = useFloatingLinkEdit(editState)

  // isOpen controls visibility for each mode
  const insertOpen = insertState.isOpen
  const editOpen = editState.isOpen && !editState.isEditing

  const insertPanel = insertOpen && (
    <div style={{ ...insertFloatingProps.style, position: 'absolute', zIndex: 50 }}>
      <div className={panelCls}>
        <FloatingLinkUrlInput className={inputCls} placeholder="Paste or type a URL…" />
        <div className="flex items-center gap-1.5">
          {/* biome-ignore lint/a11y/noLabelWithoutControl: label wraps FloatingLinkNewTabInput which renders a checkbox */}
          <label className="flex cursor-pointer items-center gap-1.5 text-[10px] text-[var(--text-muted)]">
            <FloatingLinkNewTabInput className="accent-[var(--axon-secondary)]" />
            Open in new tab
          </label>
          <button
            type="button"
            className={cn(btnCls, 'ml-auto')}
            onMouseDown={(e) => {
              e.preventDefault()
              submitFloatingLink(editor)
            }}
          >
            Apply
          </button>
        </div>
      </div>
    </div>
  )

  const editPanel = editOpen && (
    <div style={{ ...editFloatingProps.style, position: 'absolute', zIndex: 50 }}>
      <div className={panelCls}>
        <FloatingLinkUrlInput className={inputCls} placeholder="Edit URL…" />
        <div className="flex items-center gap-1">
          <button type="button" className={cn(btnCls, 'mr-auto')} {...editButtonProps}>
            Edit
          </button>
          <button
            type="button"
            className={iconBtnCls}
            title="Open link"
            onClick={() => {
              const linkNode = editor.api.node({ match: { type: editor.getType?.('a') ?? 'a' } })
              const url = (linkNode?.[0] as { url?: string } | undefined)?.url
              if (url && /^https?:\/\//i.test(url))
                window.open(url, '_blank', 'noopener,noreferrer')
            }}
          >
            <ExternalLink className="size-3.5" />
          </button>
          <button type="button" className={iconBtnCls} title="Remove link" {...unlinkButtonProps}>
            <Link2Off className="size-3.5" />
          </button>
        </div>
      </div>
    </div>
  )

  return (
    <>
      {insertPanel}
      {editPanel}
    </>
  )
}
