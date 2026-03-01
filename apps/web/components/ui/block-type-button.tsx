'use client'

import { useEditorRef, useEditorSelector } from 'platejs/react'
import type * as React from 'react'

import { ToolbarButton } from './toolbar'

export function BlockTypeButton({
  nodeType,
  ...props
}: React.ComponentProps<typeof ToolbarButton> & { nodeType: string }) {
  const editor = useEditorRef()

  const isActive = useEditorSelector(
    // biome-ignore lint/suspicious/noExplicitAny: block node type is not strongly typed
    (ed) => (ed.api.block()?.[0] as any)?.type === nodeType,
    [nodeType],
  )

  return (
    <ToolbarButton
      {...props}
      pressed={isActive}
      onMouseDown={(e) => {
        e.preventDefault()
        editor.tf.toggleBlock(nodeType)
      }}
    />
  )
}
