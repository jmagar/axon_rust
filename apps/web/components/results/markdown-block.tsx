'use client'

import { Plate, usePlateEditor } from 'platejs/react'
import { useMemo } from 'react'

import { BasicNodesKit } from '@/components/editor/plugins/basic-nodes-kit'
import { Editor, EditorContainer } from '@/components/ui/editor'
import { markdownToPlateNodes } from '@/lib/markdown'

interface MarkdownBlockProps {
  markdown: string
  className?: string
}

export function MarkdownBlock({ markdown, className }: MarkdownBlockProps) {
  const nodes = useMemo(() => markdownToPlateNodes(markdown), [markdown])

  const editor = usePlateEditor({
    plugins: BasicNodesKit,
    // biome-ignore lint/suspicious/noExplicitAny: Plate Value type mismatch with Descendant[]
    value: nodes as any,
    readOnly: true,
  })

  return (
    <Plate key={markdown} editor={editor} readOnly>
      <EditorContainer variant="default" className="h-auto border-none bg-transparent">
        <Editor variant="none" readOnly className={`px-0 py-0 ui-long-copy ${className ?? ''}`} />
      </EditorContainer>
    </Plate>
  )
}
