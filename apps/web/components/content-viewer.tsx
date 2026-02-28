'use client'

import { Check, Copy } from 'lucide-react'
import { Plate, usePlateEditor } from 'platejs/react'
import { useMemo, useState } from 'react'

import { BasicNodesKit } from '@/components/editor/plugins/basic-nodes-kit'
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
      <div className="font-mono text-[13px] leading-relaxed text-[var(--axon-secondary)]">
        <span className="mb-2 block text-sm font-bold text-[var(--destructive)]">Error</span>
        {errorMessage}
      </div>
    )
  }

  if (!markdown && !isProcessing) {
    return (
      <div className="flex h-32 items-center justify-center text-sm text-[var(--text-muted)]">
        Run a command to see results
      </div>
    )
  }

  if (!markdown && isProcessing) {
    return (
      <div className="flex items-center gap-2 text-[var(--text-muted)]">
        <span className="inline-block size-2.5 animate-spin rounded-full border-[1.5px] border-[var(--border-subtle)] border-t-[var(--axon-secondary)]" />
        <span className="text-xs">Processing...</span>
      </div>
    )
  }

  return <PlateContent markdown={markdown} />
}

function InlineCopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)

  return (
    <button
      type="button"
      onClick={() => {
        navigator.clipboard.writeText(text)
        setCopied(true)
        setTimeout(() => setCopied(false), 1500)
      }}
      className={`inline-flex items-center gap-1 rounded border px-2 py-1 text-xs transition-all duration-200 ${
        copied
          ? 'border-[rgba(130,217,160,0.4)] bg-[rgba(130,217,160,0.12)] text-[var(--axon-success)]'
          : 'border-[var(--border-subtle)] bg-[var(--surface-float)] text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
      }`}
    >
      {copied ? <Check className="size-3 animate-check-bounce" /> : <Copy className="size-3" />}
      {copied ? 'Copied' : 'Copy'}
    </button>
  )
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
      <div className="absolute right-3 top-3 z-10">
        <InlineCopyButton text={markdown} />
      </div>
      <Plate editor={editor} readOnly>
        <EditorContainer variant="default" className="h-auto">
          <Editor variant="none" readOnly className="px-0 py-0 text-sm leading-[1.75]" />
        </EditorContainer>
      </Plate>
    </div>
  )
}
