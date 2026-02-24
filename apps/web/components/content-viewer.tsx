'use client'

import { Plate, usePlateEditor } from 'platejs/react'
import { useMemo } from 'react'

import { BasicNodesKit } from '@/components/editor/plugins/basic-nodes-kit'
import { CopyButton } from '@/components/ui/copy-button'
import { Editor, EditorContainer } from '@/components/ui/editor'
import { markdownToPlateNodes } from '@/lib/markdown'

interface ContentViewerProps {
  markdown: string
  isProcessing: boolean
  errorMessage?: string
}

export function ContentViewer({ markdown, isProcessing, errorMessage }: ContentViewerProps) {
  if (errorMessage) {
    return (
      <div className="font-mono text-[13px] leading-relaxed text-[#ef4444]">
        <span className="mb-2 block text-sm font-bold text-[#ff87af]">Error</span>
        {errorMessage}
      </div>
    )
  }

  if (!markdown && !isProcessing) {
    return (
      <div className="flex h-32 items-center justify-center text-sm text-[#8787af]">
        Run a command to see results
      </div>
    )
  }

  if (!markdown && isProcessing) {
    return (
      <div className="flex items-center gap-2 text-[#8787af]">
        <span className="inline-block size-2.5 animate-spin rounded-full border-[1.5px] border-[rgba(255,135,175,0.2)] border-t-[#ff87af]" />
        <span className="text-xs">Processing...</span>
      </div>
    )
  }

  return <PlateContent markdown={markdown} />
}

function PlateContent({ markdown }: { markdown: string }) {
  const nodes = useMemo(() => markdownToPlateNodes(markdown), [markdown])

  const editor = usePlateEditor({
    plugins: BasicNodesKit,
    // Safe cast: deserializeMd produces TElement nodes, not bare TText
    // biome-ignore lint/suspicious/noExplicitAny: Plate Value type mismatch with Descendant[]
    value: nodes as any,
    readOnly: true,
  })

  return (
    <div className="relative">
      <CopyButton text={markdown} />
      <Plate editor={editor} readOnly>
        <EditorContainer variant="default" className="h-auto">
          <Editor variant="none" readOnly className="px-0 py-0 text-sm leading-[1.75]" />
        </EditorContainer>
      </Plate>
    </div>
  )
}
