'use client'

import type { JSX } from 'react'

import { CopyButton } from '@/components/ui/copy-button'

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
    ? stdoutJson.map((obj) => JSON.stringify(obj, null, 2)).join('\n')
    : stdoutLines.join('\n')

  return (
    <div className="relative">
      <CopyButton text={copyText} />
      {hasJson ? (
        <div className="space-y-3">
          {stdoutJson.map((obj, i) => (
            <JsonBlock key={i} data={obj} />
          ))}
        </div>
      ) : (
        <pre className="max-h-[60vh] overflow-auto whitespace-pre-wrap font-mono text-[12px] leading-relaxed text-[#dce6f0]">
          {stdoutLines.join('\n')}
        </pre>
      )}
    </div>
  )
}

function JsonBlock({ data }: { data: unknown }) {
  const formatted = JSON.stringify(data, null, 2)

  return (
    <pre
      className="overflow-auto rounded-lg border border-[rgba(175,215,255,0.08)] p-3 font-mono text-[12px] leading-relaxed"
      style={{ background: 'rgba(10, 18, 35, 0.4)' }}
    >
      {highlightJson(formatted)}
    </pre>
  )
}

/**
 * Simple JSON syntax highlighter using regex + spans.
 * Keys get one color, strings another, numbers/booleans/null a third.
 */
function highlightJson(json: string): (string | JSX.Element)[] {
  const parts: (string | JSX.Element)[] = []
  // Match JSON tokens: keys (quoted before colon), strings, numbers, booleans, null
  const regex =
    /("(?:[^"\\]|\\.)*")\s*:|("(?:[^"\\]|\\.)*")|(\b(?:true|false|null)\b)|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)/g
  let lastIndex = 0
  let match: RegExpExecArray | null = null

  match = regex.exec(json)
  while (match !== null) {
    // Push preceding plain text
    if (match.index > lastIndex) {
      parts.push(json.slice(lastIndex, match.index))
    }

    if (match[1] !== undefined) {
      // Key (group 1 — quoted string before colon)
      parts.push(
        <span key={`k${match.index}`} className="text-[#87afff]">
          {match[1]}
        </span>,
      )
      parts.push(':')
    } else if (match[2] !== undefined) {
      // String value
      parts.push(
        <span key={`s${match.index}`} className="text-[#87d787]">
          {match[2]}
        </span>,
      )
    } else if (match[3] !== undefined) {
      // Boolean / null
      parts.push(
        <span key={`b${match.index}`} className="text-[#ffaf87]">
          {match[3]}
        </span>,
      )
    } else if (match[4] !== undefined) {
      // Number
      parts.push(
        <span key={`n${match.index}`} className="text-[#ffaf87]">
          {match[4]}
        </span>,
      )
    }

    lastIndex = match.index + match[0].length
    match = regex.exec(json)
  }

  // Trailing text (closing braces, etc.)
  if (lastIndex < json.length) {
    parts.push(json.slice(lastIndex))
  }

  return parts
}
