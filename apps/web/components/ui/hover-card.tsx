'use client'

import { HoverCard as HoverCardPrimitive } from 'radix-ui'
import type * as React from 'react'
import { cn } from '@/lib/utils'

function HoverCard(props: React.ComponentProps<typeof HoverCardPrimitive.Root>) {
  return <HoverCardPrimitive.Root data-slot="hover-card" {...props} />
}

function HoverCardTrigger(props: React.ComponentProps<typeof HoverCardPrimitive.Trigger>) {
  return <HoverCardPrimitive.Trigger data-slot="hover-card-trigger" {...props} />
}

function HoverCardContent({
  className,
  sideOffset = 8,
  ...props
}: React.ComponentProps<typeof HoverCardPrimitive.Content>) {
  return (
    <HoverCardPrimitive.Portal>
      <HoverCardPrimitive.Content
        data-slot="hover-card-content"
        sideOffset={sideOffset}
        className={cn(
          'z-50 w-80 rounded-xl border border-[var(--border-subtle)] bg-[rgba(9,18,37,0.96)] p-4 text-[var(--text-secondary)] shadow-[var(--shadow-xl)] backdrop-blur-md',
          className,
        )}
        {...props}
      />
    </HoverCardPrimitive.Portal>
  )
}

export { HoverCard, HoverCardContent, HoverCardTrigger }
