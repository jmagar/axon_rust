'use client'

import type { Descendant } from 'platejs'
import { normalizeNodeId } from 'platejs'
import { Plate, usePlateEditor } from 'platejs/react'

import { BasicNodesKit } from '@/components/editor/plugins/basic-nodes-kit'
import { Editor, EditorContainer } from '@/components/ui/editor'

interface PlateEditorProps {
  value?: Descendant[]
  readOnly?: boolean
  variant?: 'demo' | 'default' | 'none' | 'ai' | 'aiChat' | 'fullWidth'
  placeholder?: string
  className?: string
  containerClassName?: string
}

export function PlateEditor({
  value: externalValue,
  readOnly = false,
  variant = 'demo',
  placeholder = 'Type...',
  className,
  containerClassName,
}: PlateEditorProps) {
  const editor = usePlateEditor({
    plugins: BasicNodesKit,
    // biome-ignore lint/suspicious/noExplicitAny: Plate Value type expects TElement[] but Descendant[] works at runtime
    value: (externalValue ?? DEMO_VALUE) as any,
    readOnly,
  })

  return (
    <Plate editor={editor} readOnly={readOnly}>
      <EditorContainer className={containerClassName}>
        <Editor variant={variant} placeholder={placeholder} className={className} />
      </EditorContainer>
    </Plate>
  )
}

const DEMO_VALUE = normalizeNodeId([
  { children: [{ text: 'Basic Editor' }], type: 'h1' },
  { children: [{ text: 'Heading 2' }], type: 'h2' },
  { children: [{ text: 'Heading 3' }], type: 'h3' },
  { children: [{ text: 'This is a blockquote element' }], type: 'blockquote' },
  {
    children: [
      { text: 'Basic marks: ' },
      { bold: true, text: 'bold' },
      { text: ', ' },
      { italic: true, text: 'italic' },
      { text: ', ' },
      { text: 'underline', underline: true },
      { text: ', ' },
      { strikethrough: true, text: 'strikethrough' },
      { text: '.' },
    ],
    type: 'p',
  },
])
