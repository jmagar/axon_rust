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
  const parsedFromLines = hasJson ? null : parseStructuredLinePayload(stdoutLines)

  if (!hasJson && !hasLines) {
    if (isProcessing) {
      return (
        <div className="flex items-center gap-2 text-[#8787af]">
          <span className="inline-block size-2.5 animate-spin rounded-full border-[1.5px] border-[rgba(255,135,175,0.2)] border-t-[#ff87af]" />
          <span className="text-xs">Processing...</span>
        </div>
      )
    }
    return (
      <div className="flex h-32 items-center justify-center text-sm text-[#8787af]">No output</div>
    )
  }

  const copyText = hasJson
    ? stdoutJson.map((obj) => formatStructuredText(obj)).join('\n\n')
    : parsedFromLines
      ? formatStructuredText(parsedFromLines)
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
      ) : parsedFromLines ? (
        <StructuredDataView data={parsedFromLines} />
      ) : (
        <pre className="max-h-[60vh] overflow-auto whitespace-pre-wrap font-mono text-[12px] leading-relaxed text-[#dce6f0]">
          {stdoutLines.join('\n')}
        </pre>
      )}
    </div>
  )
}

function parseStructuredLinePayload(lines: string[]): unknown | null {
  if (lines.length === 0) return null

  const joined = lines.join('\n').trim()
  if (!joined) return null

  if (joined.startsWith('{') || joined.startsWith('[')) {
    try {
      return JSON.parse(joined)
    } catch {
      // fall through
    }
  }

  if (lines.length === 1) {
    const line = lines[0].trim()
    if (line.startsWith('{') || line.startsWith('[')) {
      try {
        return JSON.parse(line)
      } catch {
        return null
      }
    }
  }

  return null
}
