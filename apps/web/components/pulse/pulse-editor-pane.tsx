'use client'

import { streamInsertChunk, useChatChunk } from '@platejs/ai/react'
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
  Quote,
  Redo2,
  Strikethrough,
  Underline,
  Undo2,
} from 'lucide-react'
import { Plate, usePlateEditor } from 'platejs/react'
import { useEffect, useRef, useState } from 'react'
import { useAIChatSetup } from '@/components/editor/plugins/ai-chat-kit'
import { CopilotKit } from '@/components/editor/plugins/copilot-kit'
import { BlockTypeButton } from '@/components/ui/block-type-button'
import { Editor, EditorContainer } from '@/components/ui/editor'
import { EditorContextMenu } from '@/components/ui/editor-context-menu'
import { FloatingLink } from '@/components/ui/floating-link'
import { FloatingToolbar } from '@/components/ui/floating-toolbar'
import { LinkToolbarButton } from '@/components/ui/link-toolbar-button'
import { ListToolbarButton } from '@/components/ui/list-toolbar-button'
import { MarkToolbarButton } from '@/components/ui/mark-toolbar-button'
import { Toolbar, ToolbarButton, ToolbarGroup } from '@/components/ui/toolbar'
import { markdownToPlateNodes } from '@/lib/markdown'

interface PulseEditorPaneProps {
  markdown: string
  onMarkdownChange: (md: string) => void
  scrollStorageKey?: string
}

/** Inner component that wires AI chat hooks requiring the Plate editor context. */
function PulseEditorInner({ editor }: { editor: ReturnType<typeof usePlateEditor> }) {
  useAIChatSetup(editor)

  useChatChunk({
    onChunk: ({ chunk }: { chunk: string }) => {
      if (editor) streamInsertChunk(editor, chunk)
    },
    onFinish: () => {
      // Leave inserted content in place for the user to review.
    },
  })

  return null
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
  const [wordCount, setWordCount] = useState(
    () =>
      markdown
        .trim()
        .split(/\s+/)
        .filter((s) => /\w/.test(s)).length,
  )

  useEffect(() => {
    const current = serializeMd(editor)
    if (current === markdown) return
    isApplyingExternalUpdateRef.current = true
    // biome-ignore lint/suspicious/noExplicitAny: Plate editor value assignment is not strongly typed
    ;(editor as any).children = markdownToPlateNodes(markdown) as any
    ;(editor as unknown as { onChange: () => void }).onChange()
    isApplyingExternalUpdateRef.current = false
    setWordCount(
      markdown
        .trim()
        .split(/\s+/)
        .filter((s) => /\w/.test(s)).length,
    )
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
    <Plate
      editor={editor}
      onChange={() => {
        if (isApplyingExternalUpdateRef.current) return
        const md = serializeMd(editor)
        onMarkdownChange(md)
        setWordCount(
          md
            .trim()
            .split(/\s+/)
            .filter((s) => /\w/.test(s)).length,
        )
      }}
    >
      {editor && <PulseEditorInner editor={editor} />}
      <div className="axon-editor flex h-full min-h-0 flex-col">
        <div
          className="bg-[rgba(10,18,35,0.32)] px-1.5 py-1"
          style={{
            backdropFilter: 'blur(8px) saturate(180%)',
            boxShadow: '0 1px 0 rgba(135, 175, 255, 0.07)',
          }}
        >
          <div className="mb-1 flex items-center justify-between px-1.5">
            <p className="ui-label flex-none">Editor</p>
            <span className="tabular-nums text-[10px] text-[var(--text-dim)]">
              {wordCount} {wordCount === 1 ? 'word' : 'words'}
            </span>
          </div>
          <Toolbar className="flex-wrap gap-0.5">
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
          </Toolbar>
        </div>
        <EditorContextMenu>
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
            variant="axon"
            className="min-h-0 flex-1"
          >
            <Editor variant="axon" placeholder="Start writing, or ask Cortex to help..." />
            <FloatingToolbar />
            <FloatingLink />
          </EditorContainer>
        </EditorContextMenu>
        <div
          className="flex shrink-0 items-center gap-2 px-2.5 py-1"
          style={{ boxShadow: '0 -1px 0 rgba(135, 175, 255, 0.07)' }}
        >
          <span className="text-[10px] text-[var(--text-dim)]">✦ AI copilot active</span>
          <span className="text-[10px] text-[var(--text-dim)] opacity-60">·</span>
          <span className="text-[10px] text-[var(--text-dim)]">
            <kbd className="font-mono">Ctrl+Space</kbd> suggest
          </span>
          <span className="text-[10px] text-[var(--text-dim)] opacity-60">·</span>
          <span className="text-[10px] text-[var(--text-dim)]">
            <kbd className="font-mono">Tab</kbd> accept
          </span>
          <span className="text-[10px] text-[var(--text-dim)] opacity-60">·</span>
          <span className="text-[10px] text-[var(--text-dim)]">
            <kbd className="font-mono">Esc</kbd> dismiss
          </span>
        </div>
      </div>
    </Plate>
  )
}
