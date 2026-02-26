'use client'

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
        <div className="flex items-center gap-2 text-[var(--axon-text-muted)]">
          <span className="inline-block size-2.5 animate-spin rounded-full border-[1.5px] border-[rgba(175,215,255,0.2)] border-t-[var(--axon-accent-pink)]" />
          <span className="text-xs">Processing...</span>
        </div>
      )
    }
    return (
      <div className="flex h-32 items-center justify-center text-sm text-[var(--axon-text-muted)]">
        No output
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
        <pre className="max-h-[60vh] overflow-auto whitespace-pre-wrap font-mono text-[12px] leading-relaxed text-[var(--axon-text-secondary)]">
          {stdoutLines.join('\n')}
        </pre>
      )}
    </div>
  )
}
