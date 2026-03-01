'use client'

import {
  getDOMSelectionBoundingClientRect,
  offset,
  useFloatingToolbar,
  useFloatingToolbarState,
} from '@platejs/floating'
import {
  Bold,
  Code2,
  Heading1,
  Heading2,
  Italic,
  Link2,
  Strikethrough,
  Underline,
} from 'lucide-react'
import { useEditorId, useEventEditorValue } from 'platejs/react'

import { BlockTypeButton } from '@/components/ui/block-type-button'
import { LinkToolbarButton } from '@/components/ui/link-toolbar-button'
import { MarkToolbarButton } from '@/components/ui/mark-toolbar-button'
import { Toolbar, ToolbarGroup, ToolbarSeparator } from '@/components/ui/toolbar'
import { cn } from '@/lib/utils'

/**
 * FloatingToolbar — balloon-style selection toolbar.
 *
 * Must be rendered inside a <Plate> context (e.g. as a child of <PlateContainer>
 * or alongside <PlateContent>). It positions itself above the current text selection
 * using @platejs/floating's virtual-element approach.
 */
export function FloatingToolbar() {
  const editorId = useEditorId()
  // `useEventEditorValue('focus')` returns the ID of whichever editor currently
  // holds the browser focus event — null when none is focused.
  const focusedEditorId = useEventEditorValue('focus')

  const state = useFloatingToolbarState({
    editorId,
    floatingOptions: {
      getBoundingClientRect: getDOMSelectionBoundingClientRect,
      middleware: [offset(6)],
      placement: 'top',
    },
    focusedEditorId,
  })

  const {
    clickOutsideRef: _clickOutsideRef,
    hidden,
    props: floatingProps,
    ref: floatingRef,
  } = useFloatingToolbar(state)

  if (hidden) return null

  // Clamp position to stay within the visible viewport (important on mobile
  // where the virtual keyboard reduces the visible area).
  const style = floatingProps.style as React.CSSProperties | undefined
  const clampedStyle: React.CSSProperties = style
    ? {
        ...style,
        top:
          typeof style.top === 'number'
            ? Math.min(style.top, (window.visualViewport?.height ?? window.innerHeight) - 60)
            : style.top,
      }
    : {}

  return (
    <div
      ref={floatingRef}
      {...floatingProps}
      style={clampedStyle}
      className={cn(
        // Layout — position is set via floatingProps.style (absolute + top/left)
        'flex items-center',
        // Visual — matches the project's glass-morphic dark theme
        'rounded-md border border-[var(--border-standard)] bg-[rgba(3,7,18,0.92)]',
        'shadow-2xl backdrop-blur-md',
        // Stack above editor content
        'z-50',
        // Prevent invisible toolbar from eating pointer events while hidden
        hidden && 'pointer-events-none',
      )}
    >
      <Toolbar className="flex items-center gap-0 px-1 py-0.5">
        {/* Block-level heading shortcuts */}
        <ToolbarGroup>
          <BlockTypeButton nodeType="h1" tooltip="Heading 1">
            <Heading1 className="size-3.5" />
          </BlockTypeButton>
          <BlockTypeButton nodeType="h2" tooltip="Heading 2">
            <Heading2 className="size-3.5" />
          </BlockTypeButton>
        </ToolbarGroup>

        <ToolbarSeparator />

        {/* Inline mark buttons */}
        <ToolbarGroup>
          <MarkToolbarButton nodeType="bold" tooltip="Bold ⌘B">
            <Bold className="size-3.5" />
          </MarkToolbarButton>
          <MarkToolbarButton nodeType="italic" tooltip="Italic ⌘I">
            <Italic className="size-3.5" />
          </MarkToolbarButton>
          <MarkToolbarButton nodeType="underline" tooltip="Underline ⌘U">
            <Underline className="size-3.5" />
          </MarkToolbarButton>
          <MarkToolbarButton nodeType="strikethrough" tooltip="Strikethrough">
            <Strikethrough className="size-3.5" />
          </MarkToolbarButton>
          <MarkToolbarButton nodeType="code" tooltip="Inline code ⌘E">
            <Code2 className="size-3.5" />
          </MarkToolbarButton>
          <LinkToolbarButton size="sm" tooltip="Link">
            <Link2 className="size-3.5" />
          </LinkToolbarButton>
        </ToolbarGroup>
      </Toolbar>
    </div>
  )
}
