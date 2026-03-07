'use client'

import type { FileUIPart, UIMessage } from 'ai'
import { ChevronLeftIcon, ChevronRightIcon, PaperclipIcon, XIcon } from 'lucide-react'
import type { ComponentProps, HTMLAttributes, ReactElement } from 'react'
import { createContext, memo, useContext, useEffect, useState } from 'react'
import { Streamdown } from 'streamdown'
import { Button } from '@/components/ui/button'
import { ButtonGroup, ButtonGroupText } from '@/components/ui/button-group'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'

export function Message({
  className,
  from,
  ...props
}: HTMLAttributes<HTMLDivElement> & { from: UIMessage['role'] }) {
  return (
    <div
      className={cn(
        'group flex w-full max-w-[95%] flex-col gap-2',
        from === 'user' ? 'is-user ml-auto justify-end' : 'is-assistant',
        className,
      )}
      {...props}
    />
  )
}

export function MessageContent({ children, className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        'flex w-fit max-w-full min-w-0 flex-col gap-2 overflow-hidden text-sm',
        'group-[.is-user]:ml-auto group-[.is-user]:rounded-2xl group-[.is-user]:bg-[rgba(255,135,175,0.14)] group-[.is-user]:px-4 group-[.is-user]:py-3',
        'group-[.is-assistant]:text-foreground',
        className,
      )}
      {...props}
    >
      {children}
    </div>
  )
}

export function MessageActions({ className, children, ...props }: ComponentProps<'div'>) {
  return (
    <div className={cn('flex items-center gap-1', className)} {...props}>
      {children}
    </div>
  )
}

export function MessageAction({
  tooltip,
  children,
  label,
  variant = 'ghost',
  size = 'icon-sm',
  ...props
}: ComponentProps<typeof Button> & { tooltip?: string; label?: string }) {
  const button = (
    <Button size={size} type="button" variant={variant} {...props}>
      {children}
      <span className="sr-only">{label || tooltip}</span>
    </Button>
  )

  if (!tooltip) return button

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>{button}</TooltipTrigger>
        <TooltipContent>
          <p>{tooltip}</p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

type MessageBranchContextValue = {
  currentBranch: number
  totalBranches: number
  goToNext: () => void
  goToPrevious: () => void
  branches: ReactElement[]
  setBranches: (branches: ReactElement[]) => void
}

const MessageBranchContext = createContext<MessageBranchContextValue | null>(null)

function useMessageBranch() {
  const context = useContext(MessageBranchContext)
  if (!context) throw new Error('MessageBranch components must be used within MessageBranch')
  return context
}

export function MessageBranch({
  defaultBranch = 0,
  onBranchChange,
  className,
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  defaultBranch?: number
  onBranchChange?: (branchIndex: number) => void
}) {
  const [currentBranch, setCurrentBranch] = useState(defaultBranch)
  const [branches, setBranches] = useState<ReactElement[]>([])

  const handleBranchChange = (nextBranch: number) => {
    setCurrentBranch(nextBranch)
    onBranchChange?.(nextBranch)
  }

  const goToPrevious = () => {
    handleBranchChange(currentBranch > 0 ? currentBranch - 1 : branches.length - 1)
  }

  const goToNext = () => {
    handleBranchChange(currentBranch < branches.length - 1 ? currentBranch + 1 : 0)
  }

  return (
    <MessageBranchContext.Provider
      value={{
        currentBranch,
        totalBranches: branches.length,
        goToNext,
        goToPrevious,
        branches,
        setBranches,
      }}
    >
      <div className={cn('grid w-full gap-2 [&>div]:pb-0', className)} {...props} />
    </MessageBranchContext.Provider>
  )
}

export function MessageBranchContent({ children, ...props }: HTMLAttributes<HTMLDivElement>) {
  const { currentBranch, setBranches, branches } = useMessageBranch()
  const childrenArray = Array.isArray(children) ? children : [children]

  useEffect(() => {
    if (branches.length !== childrenArray.length) setBranches(childrenArray as ReactElement[])
  }, [branches.length, childrenArray, setBranches])

  return childrenArray.map((branch, index) => (
    <div
      key={(branch as ReactElement).key ?? index}
      className={cn(
        'grid gap-2 overflow-hidden [&>div]:pb-0',
        index === currentBranch ? 'block' : 'hidden',
      )}
      {...props}
    >
      {branch}
    </div>
  ))
}

export function MessageBranchSelector({ ...props }: ComponentProps<typeof ButtonGroup>) {
  const { totalBranches } = useMessageBranch()
  if (totalBranches <= 1) return null
  return <ButtonGroup orientation="horizontal" {...props} />
}

export function MessageBranchPrevious({ children, ...props }: ComponentProps<typeof Button>) {
  const { goToPrevious, totalBranches } = useMessageBranch()
  return (
    <Button
      aria-label="Previous branch"
      disabled={totalBranches <= 1}
      onClick={goToPrevious}
      size="icon-sm"
      type="button"
      variant="ghost"
      {...props}
    >
      {children ?? <ChevronLeftIcon size={14} />}
    </Button>
  )
}

export function MessageBranchNext({ children, ...props }: ComponentProps<typeof Button>) {
  const { goToNext, totalBranches } = useMessageBranch()
  return (
    <Button
      aria-label="Next branch"
      disabled={totalBranches <= 1}
      onClick={goToNext}
      size="icon-sm"
      type="button"
      variant="ghost"
      {...props}
    >
      {children ?? <ChevronRightIcon size={14} />}
    </Button>
  )
}

export function MessageBranchPage({ className, ...props }: ComponentProps<'span'>) {
  const { currentBranch, totalBranches } = useMessageBranch()
  return (
    <ButtonGroupText
      className={cn('border-none bg-transparent text-muted-foreground shadow-none', className)}
      {...props}
    >
      {currentBranch + 1} of {totalBranches}
    </ButtonGroupText>
  )
}

export const MessageResponse = memo(
  function MessageResponse(props: ComponentProps<typeof Streamdown>) {
    return (
      <Streamdown
        className={cn('size-full [&>*:first-child]:mt-0 [&>*:last-child]:mb-0', props.className)}
        {...props}
      />
    )
  },
  (prevProps, nextProps) => prevProps.children === nextProps.children,
)

export function MessageAttachment({
  data,
  className,
  onRemove,
  ...props
}: HTMLAttributes<HTMLDivElement> & { data: FileUIPart; onRemove?: () => void }) {
  const filename = data.filename || ''
  const isImage = Boolean(data.mediaType?.startsWith('image/') && data.url)
  const attachmentLabel = filename || (isImage ? 'Image' : 'Attachment')

  return (
    <div className={cn('group relative size-24 overflow-hidden rounded-lg', className)} {...props}>
      {isImage ? (
        <>
          <img alt={filename || 'attachment'} className="size-full object-cover" src={data.url} />
          {onRemove ? (
            <Button
              aria-label="Remove attachment"
              className="absolute right-2 top-2 size-6 rounded-full bg-background/80 p-0 opacity-0 backdrop-blur-sm transition-opacity group-hover:opacity-100 [&>svg]:size-3"
              onClick={(event) => {
                event.stopPropagation()
                onRemove()
              }}
              type="button"
              variant="ghost"
            >
              <XIcon />
            </Button>
          ) : null}
        </>
      ) : (
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <div className="flex size-full items-center justify-center rounded-lg bg-muted text-muted-foreground">
                <PaperclipIcon className="size-4" />
              </div>
            </TooltipTrigger>
            <TooltipContent>
              <p>{attachmentLabel}</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      )}
    </div>
  )
}

export function MessageAttachments({ children, className, ...props }: ComponentProps<'div'>) {
  if (!children) return null
  return (
    <div className={cn('ml-auto flex w-fit flex-wrap items-start gap-2', className)} {...props}>
      {children}
    </div>
  )
}

export function MessageToolbar({ className, children, ...props }: ComponentProps<'div'>) {
  return (
    <div
      className={cn('mt-4 flex w-full items-center justify-between gap-4', className)}
      {...props}
    >
      {children}
    </div>
  )
}
