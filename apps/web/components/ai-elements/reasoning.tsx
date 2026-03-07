'use client'

import { useControllableState } from '@radix-ui/react-use-controllable-state'
import { BrainIcon, ChevronDownIcon } from 'lucide-react'
import type { ComponentProps, ReactNode } from 'react'
import { createContext, memo, useContext, useEffect, useState } from 'react'
import { Streamdown } from 'streamdown'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { cn } from '@/lib/utils'
import { Shimmer } from './shimmer'

type ReasoningContextValue = {
  isStreaming: boolean
  isOpen: boolean
  duration: number | undefined
}

const ReasoningContext = createContext<ReasoningContextValue | null>(null)

function useReasoning() {
  const context = useContext(ReasoningContext)
  if (!context) throw new Error('Reasoning components must be used within Reasoning')
  return context
}

const AUTO_CLOSE_DELAY = 1000
const MS_IN_S = 1000

export function Reasoning({
  className,
  isStreaming = false,
  open,
  defaultOpen = true,
  onOpenChange,
  duration: durationProp,
  children,
  ...props
}: ComponentProps<typeof Collapsible> & {
  isStreaming?: boolean
  duration?: number
}) {
  const [isOpen, setIsOpen] = useControllableState({
    prop: open,
    defaultProp: defaultOpen,
    onChange: onOpenChange,
  })
  const [duration, setDuration] = useControllableState({
    prop: durationProp,
    defaultProp: undefined,
  })
  const [hasAutoClosed, setHasAutoClosed] = useState(false)
  const [startTime, setStartTime] = useState<number | null>(null)

  useEffect(() => {
    if (isStreaming) {
      if (startTime === null) setStartTime(Date.now())
      return
    }

    if (startTime !== null) {
      setDuration(Math.ceil((Date.now() - startTime) / MS_IN_S))
      setStartTime(null)
    }
  }, [isStreaming, setDuration, startTime])

  useEffect(() => {
    if (!defaultOpen || isStreaming || !isOpen || hasAutoClosed) return
    const timer = window.setTimeout(() => {
      setIsOpen(false)
      setHasAutoClosed(true)
    }, AUTO_CLOSE_DELAY)
    return () => window.clearTimeout(timer)
  }, [defaultOpen, hasAutoClosed, isOpen, isStreaming, setIsOpen])

  return (
    <ReasoningContext.Provider value={{ isStreaming, isOpen, duration }}>
      <Collapsible
        className={cn(
          'not-prose rounded-2xl border border-[rgba(135,175,255,0.12)] bg-[rgba(7,12,26,0.6)] p-3',
          className,
        )}
        onOpenChange={setIsOpen}
        open={isOpen}
        {...props}
      >
        {children}
      </Collapsible>
    </ReasoningContext.Provider>
  )
}

export function ReasoningTrigger({
  className,
  children,
  getThinkingMessage,
  ...props
}: ComponentProps<typeof CollapsibleTrigger> & {
  getThinkingMessage?: (isStreaming: boolean, duration?: number) => ReactNode
}) {
  const { isOpen, isStreaming, duration } = useReasoning()
  const message =
    getThinkingMessage?.(isStreaming, duration) ??
    (isStreaming ? (
      <Shimmer duration={1}>Thinking...</Shimmer>
    ) : (
      <p>Thought for {duration ?? 'a few'} seconds</p>
    ))

  return (
    <CollapsibleTrigger
      className={cn(
        'flex w-full items-center gap-2 text-sm text-muted-foreground transition-colors hover:text-foreground',
        className,
      )}
      {...props}
    >
      {children ?? (
        <>
          <BrainIcon className="size-4" />
          {message}
          <ChevronDownIcon
            className={cn('ml-auto size-4 transition-transform', isOpen ? 'rotate-180' : '')}
          />
        </>
      )}
    </CollapsibleTrigger>
  )
}

export const ReasoningContent = memo(function ReasoningContent({
  className,
  children,
  ...props
}: ComponentProps<typeof CollapsibleContent> & { children: string }) {
  return (
    <CollapsibleContent
      className={cn(
        'mt-3 text-sm text-muted-foreground data-[state=closed]:animate-out data-[state=open]:animate-in',
        className,
      )}
      {...props}
    >
      <Streamdown>{children}</Streamdown>
    </CollapsibleContent>
  )
})
