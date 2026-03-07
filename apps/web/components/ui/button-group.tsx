'use client'

import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

export function ButtonGroup({
  className,
  orientation = 'horizontal',
  ...props
}: ComponentProps<'div'> & { orientation?: 'horizontal' | 'vertical' }) {
  return (
    <div
      className={cn(
        'inline-flex overflow-hidden rounded-md border border-border bg-[rgba(7,12,26,0.72)] shadow-xs',
        orientation === 'vertical' ? 'flex-col' : 'flex-row',
        className,
      )}
      role="group"
      {...props}
    />
  )
}

export function ButtonGroupText({ className, ...props }: ComponentProps<'span'>) {
  return (
    <span
      className={cn(
        'inline-flex min-h-8 items-center px-2 text-xs text-muted-foreground',
        className,
      )}
      {...props}
    />
  )
}
