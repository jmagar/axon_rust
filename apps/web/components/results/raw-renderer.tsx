'use client'

import { TerminalSquare } from 'lucide-react'
import { StructuredDataView } from '@/components/results/structured-data-view'
import { CopyButton } from '@/components/ui/copy-button'
import { formatStructuredText } from '@/lib/structured-text'

interface RawRendererProps {
  stdoutJson: unknown[]
  stdoutLines: string[]
  isProcessing?: boolean
}

export function RawRenderer({ stdoutJson, stdoutLines, isProcessing }: RawRendererProps) {
  const hasJson = stdoutJson.length > 0
  const hasLines = stdoutLines.length > 0

  if (!hasJson && !hasLines) {
    if (isProcessing) {
      return (
        <div className="flex h-40 flex-col items-center justify-center gap-3 animate-fade-in">
          <div className="flex gap-1">
            {[0, 1, 2].map((i) => (
              <span
                key={i}
                className="inline-block size-1.5 rounded-full bg-[var(--axon-primary)]"
                style={{ animation: `breathing 1.4s ease-in-out ${i * 180}ms infinite` }}
              />
            ))}
          </div>
          <div className="text-center">
            <p className="text-sm text-[var(--text-secondary)] animate-breathing">Processing...</p>
            <p className="text-xs text-[var(--text-muted)] mt-0.5">
              Large operations may take several minutes
            </p>
          </div>
        </div>
      )
    }
    return (
      <div className="flex h-40 flex-col items-center justify-center gap-3">
        <div className="size-8 rounded-full border border-[var(--border-subtle)] bg-[var(--surface-elevated)] flex items-center justify-center">
          <TerminalSquare className="size-4 text-[var(--text-dim)]" />
        </div>
        <div className="text-center">
          <p className="text-sm font-medium text-[var(--text-secondary)]">No output yet</p>
          <p className="text-xs text-[var(--text-muted)] mt-0.5">
            Run a command to see results here
          </p>
        </div>
      </div>
    )
  }

  const copyText = hasJson
    ? stdoutJson.map((obj) => formatStructuredText(obj)).join('\n\n')
    : stdoutLines.join('\n')

  return (
    <div className="relative">
      <CopyButton text={copyText} />
      {hasJson ? (
        <div className="space-y-3">
          {stdoutJson.map((obj, i) => (
            <StructuredDataView key={i} data={obj} />
          ))}
        </div>
      ) : (
        <pre className="max-h-[60vh] overflow-auto whitespace-pre-wrap ui-mono text-[var(--text-secondary)]">
          {stdoutLines.join('\n')}
        </pre>
      )}
    </div>
  )
}
