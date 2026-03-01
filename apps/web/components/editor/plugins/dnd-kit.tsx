'use client'

import { DndPlugin } from '@platejs/dnd'

import { BlockDraggable } from '@/components/ui/block-draggable'

export const DndKit = [
  DndPlugin.configure({
    options: { enableScroller: true },
    render: { aboveNodes: BlockDraggable },
  }),
]
