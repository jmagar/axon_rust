'use client'

import { Bold, Code2, Italic, Strikethrough, Underline } from 'lucide-react'
import { serializeMd } from '@platejs/markdown'
import { Plate, usePlateEditor } from 'platejs/react'
import { useEffect, useRef } from 'react'
import { CopilotKit } from '@/components/editor/plugins/copilot-kit'
import { Editor, EditorContainer } from '@/components/ui/editor'
import { MarkToolbarButton } from '@/components/ui/mark-toolbar-button'
import { Toolbar, ToolbarGroup } from '@/components/ui/toolbar'
import { markdownToPlateNodes } from '@/lib/markdown'

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

  useEffect(() => {
    const current = serializeMd(editor)
    if (current === markdown) return
    isApplyingExternalUpdateRef.current = true
    // biome-ignore lint/suspicious/noExplicitAny: Plate editor value assignment is not strongly typed
    ;(editor as any).children = markdownToPlateNodes(markdown) as any
    ;((editor as unknown) as { onChange: () => void }).onChange()
    isApplyingExternalUpdateRef.current = false
  }, [editor, markdown])

  useEffect(() => {
    const node = editorScrollRef.current
    if (!node) return
    try {
      const saved = Number(window.localStorage.getItem(scrollStorageKey) ?? 0)
      if (Number.isFinite(saved) && saved > 0) node.scrollTop = saved
    } catch {
      // Ignore storage restore failures.
    }
  }, [scrollStorageKey])

  return (
    <Plate
      editor={editor}
      onChange={() => {
        if (isApplyingExternalUpdateRef.current) return
        const md = serializeMd(editor)
        onMarkdownChange(md)
      }}
    >
      <div className="flex h-full min-h-0 flex-col">
        <div className="border-b border-[rgba(255,135,175,0.1)] bg-[rgba(10,18,35,0.32)] px-1.5 py-1">
          <Toolbar className="flex-wrap gap-0.5">
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
              <MarkToolbarButton nodeType="code" tooltip="Code (Ctrl+E)">
                <Code2 className="size-3.5" />
              </MarkToolbarButton>
            </ToolbarGroup>
          </Toolbar>
        </div>
        <EditorContainer
          ref={editorScrollRef}
          onScroll={() => {
            if (!editorScrollRef.current) return
            try {
              window.localStorage.setItem(scrollStorageKey, String(editorScrollRef.current.scrollTop))
            } catch {
              // Ignore storage failures.
            }
          }}
          className="min-h-0 flex-1"
        >
          <Editor
            variant="none"
            placeholder="Start writing, or ask Pulse to help..."
            className="min-h-full px-3 py-2.5 text-[length:var(--text-md)] leading-[var(--leading-copy)]"
          />
        </EditorContainer>
      </div>
    </Plate>
  )
}
