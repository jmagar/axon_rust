'use client'

import { serializeMd } from '@platejs/markdown'
import { Plate, usePlateEditor } from 'platejs/react'
import { CopilotKit } from '@/components/editor/plugins/copilot-kit'
import { Editor, EditorContainer } from '@/components/ui/editor'
import { markdownToPlateNodes } from '@/lib/markdown'

interface PulseEditorPaneProps {
  markdown: string
  onMarkdownChange: (md: string) => void
}

export function PulseEditorPane({ markdown, onMarkdownChange }: PulseEditorPaneProps) {
  const editor = usePlateEditor({
    plugins: CopilotKit,
    // biome-ignore lint/suspicious/noExplicitAny: Plate value typing mismatch with Descendant[]
    value: markdownToPlateNodes(markdown) as any,
  })

  return (
    <Plate
      editor={editor}
      onChange={() => {
        const md = serializeMd(editor)
        onMarkdownChange(md)
      }}
    >
      <EditorContainer className="h-full">
        <Editor
          variant="fullWidth"
          placeholder="Start writing, or ask Pulse to help..."
          className="min-h-[50vh] p-4 text-sm"
        />
      </EditorContainer>
    </Plate>
  )
}
