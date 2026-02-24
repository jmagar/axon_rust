'use client'

import { forwardRef, useCallback, useImperativeHandle, useRef, useState } from 'react'
import { useAxonWs } from '@/hooks/use-axon-ws'
import { MODES, NO_INPUT_MODES, type ModeId } from '@/lib/ws-protocol'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

interface OmniboxProps {
  onExecute?: (mode: string, input: string) => void
  onDone?: () => void
}

export interface OmniboxHandle {
  handleDone: (elapsed_ms: number, exit_code: number) => void
  handleError: (message: string, elapsed_ms?: number) => void
}

export const Omnibox = forwardRef<OmniboxHandle, OmniboxProps>(
  function Omnibox({ onExecute, onDone }, ref) {
    const { send, status } = useAxonWs()
    const [mode, setMode] = useState<ModeId>('scrape')
    const [input, setInput] = useState('')
    const [isProcessing, setIsProcessing] = useState(false)
    const [statusText, setStatusText] = useState('')
    const inputRef = useRef<HTMLInputElement>(null)
    const startTimeRef = useRef(0)
    // Track a simple execution ID for cancel messages
    const execIdRef = useRef(0)

    const currentMode = MODES.find((m) => m.id === mode) ?? MODES[0]

    const executeCommand = useCallback(
      (execMode: ModeId, execInput: string) => {
        if (status !== 'connected') return
        if (isProcessing) return
        if (!execInput.trim() && !(NO_INPUT_MODES as ReadonlySet<string>).has(execMode)) return

        execIdRef.current += 1
        setIsProcessing(true)
        startTimeRef.current = Date.now()
        setStatusText('processing...')

        send({
          type: 'execute',
          mode: execMode,
          input: execInput.trim(),
          flags: {},
        })

        onExecute?.(execMode, execInput.trim())
      },
      [status, isProcessing, send, onExecute],
    )

    const execute = useCallback(() => {
      executeCommand(mode, input)
    }, [executeCommand, mode, input])

    const cancel = useCallback(() => {
      if (!isProcessing) return
      send({ type: 'cancel', id: String(execIdRef.current) })
      setIsProcessing(false)
      const elapsed = Date.now() - startTimeRef.current
      const secs = (elapsed / 1000).toFixed(1)
      setStatusText(`${secs}s · cancelled`)
    }, [isProcessing, send])

    const selectMode = useCallback(
      (id: ModeId) => {
        setMode(id)
        if ((NO_INPUT_MODES as ReadonlySet<string>).has(id)) {
          // Auto-execute modes that need no input.
          // Defer so React state settles before we check isProcessing.
          setTimeout(() => {
            executeCommand(id, '')
          }, 0)
        } else {
          inputRef.current?.focus()
        }
      },
      [executeCommand],
    )

    const handleDone = useCallback(
      (elapsed_ms: number, exit_code: number) => {
        setIsProcessing(false)
        const secs = (elapsed_ms / 1000).toFixed(1)
        setStatusText(`${secs}s · exit ${exit_code}`)
        onDone?.()
      },
      [onDone],
    )

    const handleError = useCallback(
      (message: string, elapsed_ms?: number) => {
        setIsProcessing(false)
        const secs = elapsed_ms ? `${(elapsed_ms / 1000).toFixed(1)}s · ` : ''
        setStatusText(`${secs}error: ${message}`)
        onDone?.()
      },
      [onDone],
    )

    useImperativeHandle(ref, () => ({ handleDone, handleError }), [
      handleDone,
      handleError,
    ])

    const handleKeyDown = useCallback(
      (e: React.KeyboardEvent) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          execute()
        }
      },
      [execute],
    )

    return (
      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                className="min-w-[120px] justify-start gap-2 border-border/50 bg-card/50"
              >
                <svg
                  className="size-4 shrink-0"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth={2}
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <path d={currentMode.icon} />
                </svg>
                <span className="truncate">{currentMode.label}</span>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent
              align="start"
              className="max-h-[400px] overflow-y-auto"
            >
              {MODES.map((m) => (
                <DropdownMenuItem
                  key={m.id}
                  onSelect={() => selectMode(m.id)}
                  className="gap-2"
                >
                  <svg
                    className="size-4 shrink-0 text-muted-foreground"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={2}
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d={m.icon} />
                  </svg>
                  <span>{m.label}</span>
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>

          <Input
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={
              (NO_INPUT_MODES as ReadonlySet<string>).has(mode)
                ? `Run ${currentMode.label}...`
                : 'Enter URL or query...'
            }
            className="flex-1 border-border/50 bg-card/50"
            disabled={isProcessing}
          />

          <Button
            onClick={isProcessing ? cancel : execute}
            disabled={
              !isProcessing &&
              (status !== 'connected' ||
                (!input.trim() && !(NO_INPUT_MODES as ReadonlySet<string>).has(mode)))
            }
            size="sm"
            className={
              isProcessing
                ? 'bg-destructive text-destructive-foreground hover:bg-destructive/90'
                : 'bg-primary text-primary-foreground hover:bg-primary/90'
            }
          >
            {isProcessing ? (
              <svg
                className="size-4 animate-spin"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path d="M12 2v4m0 12v4m-7.071-15.071l2.828 2.828m8.486 8.486l2.828 2.828M2 12h4m12 0h4M4.929 19.071l2.828-2.828m8.486-8.486l2.828-2.828" />
              </svg>
            ) : (
              'Run'
            )}
          </Button>
        </div>

        {statusText && (
          <div className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
            {isProcessing && (
              <span className="size-1.5 animate-pulse rounded-full bg-primary" />
            )}
            <span>{statusText}</span>
          </div>
        )}
      </div>
    )
  },
)
