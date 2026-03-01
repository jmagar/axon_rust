'use client'

import { useListToolbarButton, useListToolbarButtonState } from '@platejs/list/react'
import type * as React from 'react'

import { ToolbarButton } from './toolbar'

export function ListToolbarButton({
  nodeType = 'disc',
  ...props
}: React.ComponentProps<typeof ToolbarButton> & {
  nodeType?: string
}) {
  const state = useListToolbarButtonState({ nodeType })
  const { props: buttonProps } = useListToolbarButton(state)
  return <ToolbarButton {...props} {...buttonProps} />
}
