'use client'

import { BlockMenuPlugin, BlockSelectionPlugin } from '@platejs/selection/react'
import type { RenderNodeWrapper } from 'platejs/react'

import { BlockSelection } from '@/components/ui/block-selection'

export const SelectionKit = [
  BlockMenuPlugin,
  BlockSelectionPlugin.configure({
    // biome-ignore lint/suspicious/noExplicitAny: Plate.js render config type mismatch
    render: { aboveNodes: BlockSelection as unknown as RenderNodeWrapper } as any,
  }),
]
