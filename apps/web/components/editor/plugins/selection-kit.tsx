'use client'

import { BlockSelectionPlugin } from '@platejs/selection/react'

import { BlockSelection } from '@/components/ui/block-selection'

export const SelectionKit = [
  BlockSelectionPlugin.configure({
    // biome-ignore lint/suspicious/noExplicitAny: shadcn component uses PlateElementProps, platejs aboveNodes expects RenderNodeWrapperProps
    render: { aboveNodes: BlockSelection as any },
  }),
]
