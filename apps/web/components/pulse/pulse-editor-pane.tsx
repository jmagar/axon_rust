'use client'

import { useListToolbarButton, useListToolbarButtonState } from '@platejs/list/react'
import { serializeMd } from '@platejs/markdown'
import {
  Bold,
  Braces,
  Code2,
  Heading1,
  Heading2,
  Heading3,
  Italic,
  Link2,
  List,
  ListOrdered,
  MoreHorizontal,
  Quote,
  Redo2,
  Sparkles,
  Strikethrough,
  Underline,
  Undo2,
} from 'lucide-react'
import { Plate, useEditorRef, usePlateEditor } from 'platejs/react'
import { useEffect, useRef, useState } from 'react'
import { DndProvider } from 'react-dnd'
import { HTML5Backend } from 'react-dnd-html5-backend'
import { CopilotKit } from '@/components/editor/plugins/copilot-kit'
import { AIToolbarButton } from '@/components/ui/ai-toolbar-button'
import { BlockContextMenu } from '@/components/ui/block-context-menu'
import { BlockTypeButton } from '@/components/ui/block-type-button'
import { CommentToolbarButton } from '@/components/ui/comment-toolbar-button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Editor, EditorContainer } from '@/components/ui/editor'
import { ExportToolbarButton } from '@/components/ui/export-toolbar-button'
import { FloatingLink } from '@/components/ui/floating-link'
import { FloatingToolbar } from '@/components/ui/floating-toolbar'
import { LinkToolbarButton } from '@/components/ui/link-toolbar-button'
import { ListToolbarButton } from '@/components/ui/list-toolbar-button'
import { MarkToolbarButton } from '@/components/ui/mark-toolbar-button'
import { Toolbar, ToolbarButton, ToolbarGroup } from '@/components/ui/toolbar'
import { markdownToPlateNodes } from '@/lib/markdown'

function countWords(text: string): number {
  return text
    .trim()
    .split(/\s+/)
    .filter((s) => /\w/.test(s)).length
}

interface PulseEditorPaneProps {
  markdown: string
  onMarkdownChange: (md: string) => void
  scrollStorageKey?: string
}

export function PulseEditorPane({
  markdown,
  onMarkdownChange,
  scrollStorageKey = 'axon.web.pulse.editor-scroll',
}: PulseEditorPaneProps) {
  const editor = usePlateEditor({
    plugins: CopilotKit,
    // biome-ignore lint/suspicious/noExplicitAny: Plate value typing mismatch with Descendant[]
    value: markdownToPlateNodes(markdown) as any,
  })
  const isApplyingExternalUpdateRef = useRef(false)
  const editorScrollRef = useRef<HTMLDivElement | null>(null)
  const scrollSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [wordCount, setWordCount] = useState(() => countWords(markdown))

  useEffect(() => {
    const current = serializeMd(editor)
    if (current === markdown) return
    isApplyingExternalUpdateRef.current = true
    // biome-ignore lint/suspicious/noExplicitAny: Plate editor value assignment is not strongly typed
    ;(editor as any).children = markdownToPlateNodes(markdown) as any
    ;(editor as unknown as { onChange: () => void }).onChange()
    isApplyingExternalUpdateRef.current = false
    setWordCount(countWords(markdown))
  }, [editor, markdown])

  // Defer scroll restore one frame so content has rendered before we set scrollTop.
  useEffect(() => {
    const node = editorScrollRef.current
    if (!node) return
    const timerId = setTimeout(() => {
      try {
        const saved = Number(window.localStorage.getItem(scrollStorageKey) ?? 0)
        if (Number.isFinite(saved) && saved > 0) node.scrollTop = saved
      } catch {
        // Ignore storage restore failures.
      }
    }, 0)
    return () => clearTimeout(timerId)
  }, [scrollStorageKey])

  // Cleanup debounce timer on unmount.
  useEffect(() => {
    return () => {
      if (scrollSaveTimerRef.current) clearTimeout(scrollSaveTimerRef.current)
    }
  }, [])

  return (
    <DndProvider backend={HTML5Backend}>
      <Plate
        editor={editor}
        onChange={() => {
          if (isApplyingExternalUpdateRef.current) return
          const md = serializeMd(editor)
          onMarkdownChange(md)
          setWordCount(countWords(md))
        }}
      >
        <div className="axon-editor flex h-full min-h-0 flex-col">
          {/* ── Desktop toolbar (hidden on mobile) ─────────────────────────────── */}
          <div
            className="bg-[rgba(10,18,35,0.32)] px-1.5 py-1"
            style={{
              backdropFilter: 'blur(8px) saturate(180%)',
              boxShadow: '0 1px 0 rgba(135, 175, 255, 0.07)',
            }}
          >
            <div className="mb-1 flex items-center justify-between px-1.5">
              <p className="ui-label flex-none">Editor</p>
            </div>

            {/* Mobile compact toolbar */}
            <Toolbar className="flex items-center gap-0.5 sm:hidden">
              <AIToolbarButton size="sm" tooltip="AI (Ctrl+J)">
                <Sparkles className="size-3.5" />
              </AIToolbarButton>
              <MarkToolbarButton nodeType="bold" tooltip="Bold (Ctrl+B)">
                <Bold className="size-3.5" />
              </MarkToolbarButton>
              <MarkToolbarButton nodeType="italic" tooltip="Italic (Ctrl+I)">
                <Italic className="size-3.5" />
              </MarkToolbarButton>
              <LinkToolbarButton size="sm" tooltip="Link (Ctrl+K)">
                <Link2 className="size-3.5" />
              </LinkToolbarButton>
              <MoreFormattingDropdown />
            </Toolbar>

            {/* Desktop full toolbar */}
            <Toolbar className="hidden flex-wrap gap-0.5 sm:flex">
              <ToolbarGroup>
                <ToolbarButton
                  size="sm"
                  tooltip="Undo (Ctrl+Z)"
                  onMouseDown={(e) => {
                    e.preventDefault()
                    editor.undo()
                  }}
                >
                  <Undo2 className="size-3.5" />
                </ToolbarButton>
                <ToolbarButton
                  size="sm"
                  tooltip="Redo (Ctrl+Y)"
                  onMouseDown={(e) => {
                    e.preventDefault()
                    editor.redo()
                  }}
                >
                  <Redo2 className="size-3.5" />
                </ToolbarButton>
              </ToolbarGroup>
              <ToolbarGroup>
                <AIToolbarButton size="sm" tooltip="AI (Ctrl+J)">
                  <Sparkles className="size-3.5" />
                </AIToolbarButton>
              </ToolbarGroup>
              <ToolbarGroup>
                <BlockTypeButton nodeType="h1" tooltip="Heading 1 (Ctrl+Alt+1)">
                  <Heading1 className="size-3.5" />
                </BlockTypeButton>
                <BlockTypeButton nodeType="h2" tooltip="Heading 2 (Ctrl+Alt+2)">
                  <Heading2 className="size-3.5" />
                </BlockTypeButton>
                <BlockTypeButton nodeType="h3" tooltip="Heading 3 (Ctrl+Alt+3)">
                  <Heading3 className="size-3.5" />
                </BlockTypeButton>
                <BlockTypeButton nodeType="blockquote" tooltip="Quote (Ctrl+Shift+.)">
                  <Quote className="size-3.5" />
                </BlockTypeButton>
                <BlockTypeButton nodeType="code_block" tooltip="Code Block (```)">
                  <Braces className="size-3.5" />
                </BlockTypeButton>
              </ToolbarGroup>
              <ToolbarGroup>
                <ListToolbarButton nodeType="disc" tooltip="Bullet List">
                  <List className="size-3.5" />
                </ListToolbarButton>
                <ListToolbarButton nodeType="decimal" tooltip="Numbered List">
                  <ListOrdered className="size-3.5" />
                </ListToolbarButton>
              </ToolbarGroup>
              <ToolbarGroup>
                <MarkToolbarButton nodeType="bold" tooltip="Bold (Ctrl+B)">
                  <Bold className="size-3.5" />
                </MarkToolbarButton>
                <MarkToolbarButton nodeType="italic" tooltip="Italic (Ctrl+I)">
                  <Italic className="size-3.5" />
                </MarkToolbarButton>
                <MarkToolbarButton nodeType="underline" tooltip="Underline (Ctrl+U)">
                  <Underline className="size-3.5" />
                </MarkToolbarButton>
                <MarkToolbarButton nodeType="strikethrough" tooltip="Strike (Ctrl+Shift+X)">
                  <Strikethrough className="size-3.5" />
                </MarkToolbarButton>
                <MarkToolbarButton nodeType="code" tooltip="Inline code (Ctrl+E)">
                  <Code2 className="size-3.5" />
                </MarkToolbarButton>
                <LinkToolbarButton size="sm" tooltip="Link (Ctrl+K)">
                  <Link2 className="size-3.5" />
                </LinkToolbarButton>
              </ToolbarGroup>
              <ToolbarGroup>
                <CommentToolbarButton />
                <ExportToolbarButton />
              </ToolbarGroup>
            </Toolbar>
          </div>

          <BlockContextMenu>
            <EditorContainer
              ref={editorScrollRef}
              onScroll={() => {
                if (!editorScrollRef.current) return
                if (scrollSaveTimerRef.current) clearTimeout(scrollSaveTimerRef.current)
                scrollSaveTimerRef.current = setTimeout(() => {
                  try {
                    window.localStorage.setItem(
                      scrollStorageKey,
                      String(editorScrollRef.current?.scrollTop ?? 0),
                    )
                  } catch {
                    // Ignore storage failures.
                  }
                }, 200)
              }}
              variant="default"
              className="min-h-0 flex-1"
            >
              <Editor variant="default" placeholder="Start writing, or ask Cortex to help..." />
              <FloatingToolbar />
              <FloatingLink />
            </EditorContainer>
          </BlockContextMenu>

          {/* ── Desktop footer ──────────────────────────────────────────────────── */}
          <div
            className="hidden shrink-0 items-center gap-2 px-2.5 py-1 sm:flex"
            style={{ boxShadow: '0 -1px 0 rgba(135, 175, 255, 0.07)' }}
          >
            <span className="inline-flex items-center gap-1 text-[10px] text-[var(--text-dim)]">
              <Sparkles className="size-2.5" />
              AI copilot active
            </span>
            <span className="text-[10px] text-[var(--text-dim)] opacity-60">·</span>
            <span className="text-[10px] text-[var(--text-dim)]">
              <kbd className="rounded border border-[var(--border-subtle)] bg-[var(--surface-primary)] px-1 font-mono text-[length:var(--text-2xs)] text-[var(--text-dim)]">
                Ctrl+Space
              </kbd>{' '}
              suggest
            </span>
            <span className="text-[10px] text-[var(--text-dim)] opacity-60">·</span>
            <span className="text-[10px] text-[var(--text-dim)]">
              <kbd className="rounded border border-[var(--border-subtle)] bg-[var(--surface-primary)] px-1 font-mono text-[length:var(--text-2xs)] text-[var(--text-dim)]">
                Tab
              </kbd>{' '}
              accept
            </span>
            <span className="text-[10px] text-[var(--text-dim)] opacity-60">·</span>
            <span className="text-[10px] text-[var(--text-dim)]">
              <kbd className="rounded border border-[var(--border-subtle)] bg-[var(--surface-primary)] px-1 font-mono text-[length:var(--text-2xs)] text-[var(--text-dim)]">
                Esc
              </kbd>{' '}
              dismiss
            </span>
            <span className="text-[10px] text-[var(--text-dim)] opacity-60">·</span>
            <span className="tabular-nums text-[10px] text-[var(--text-dim)]">
              {wordCount} {wordCount === 1 ? 'word' : 'words'}
            </span>
          </div>

          {/* ── Mobile footer ───────────────────────────────────────────────────── */}
          <Toolbar
            className="shrink-0 gap-2 px-2.5 py-1.5 sm:hidden pb-[env(safe-area-inset-bottom)]"
            style={{ boxShadow: '0 -1px 0 rgba(135, 175, 255, 0.07)' }}
          >
            <AIToolbarButton size="sm" tooltip="AI">
              <Sparkles className="size-4" />
            </AIToolbarButton>
            <CommentToolbarButton />
            <ExportToolbarButton />
          </Toolbar>
        </div>
      </Plate>
    </DndProvider>
  )
}

/** Mobile overflow dropdown — remaining formatting options not shown in compact toolbar. */
function MoreFormattingDropdown() {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <ToolbarButton size="sm" tooltip="More formatting">
          <MoreHorizontal className="size-3.5" />
        </ToolbarButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent side="top" align="end" className="w-44">
        <MoreFormattingItems />
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

/** Plate context is required for editor hooks — must be a separate component rendered inside <Plate>. */
function MoreFormattingItems() {
  const editor = useEditorRef()
  const discListState = useListToolbarButtonState({ nodeType: 'disc' })
  const { props: discListProps } = useListToolbarButton(discListState)
  const decimalListState = useListToolbarButtonState({ nodeType: 'decimal' })
  const { props: decimalListProps } = useListToolbarButton(decimalListState)

  function toggleBlock(type: string) {
    editor.tf.toggleBlock(type)
  }

  function toggleMark(type: string) {
    editor.tf.toggleMark(type)
  }

  return (
    <>
      <DropdownMenuItem onSelect={() => toggleBlock('h1')}>
        <Heading1 className="mr-2 size-4" /> Heading 1
      </DropdownMenuItem>
      <DropdownMenuItem onSelect={() => toggleBlock('h2')}>
        <Heading2 className="mr-2 size-4" /> Heading 2
      </DropdownMenuItem>
      <DropdownMenuItem onSelect={() => toggleBlock('h3')}>
        <Heading3 className="mr-2 size-4" /> Heading 3
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem onSelect={() => toggleMark('underline')}>
        <Underline className="mr-2 size-4" /> Underline
      </DropdownMenuItem>
      <DropdownMenuItem onSelect={() => toggleMark('strikethrough')}>
        <Strikethrough className="mr-2 size-4" /> Strikethrough
      </DropdownMenuItem>
      <DropdownMenuItem onSelect={() => toggleMark('code')}>
        <Code2 className="mr-2 size-4" /> Inline code
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem onSelect={() => discListProps.onClick?.()}>
        <List className="mr-2 size-4" /> Bullet list
      </DropdownMenuItem>
      <DropdownMenuItem onSelect={() => decimalListProps.onClick?.()}>
        <ListOrdered className="mr-2 size-4" /> Numbered list
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem onSelect={() => toggleBlock('blockquote')}>
        <Quote className="mr-2 size-4" /> Quote
      </DropdownMenuItem>
      <DropdownMenuItem onSelect={() => toggleBlock('code_block')}>
        <Braces className="mr-2 size-4" /> Code block
      </DropdownMenuItem>
    </>
  )
}
