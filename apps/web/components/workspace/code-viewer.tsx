'use client'

import { Check, Copy } from 'lucide-react'
import { useCallback, useState } from 'react'

interface CodeViewerProps {
  content: string
  language?: string
  fileName?: string
}

export function CodeViewer({ content, language, fileName }: CodeViewerProps) {
  const [copied, setCopied] = useState(false)

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(content).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [content])

  const lines = content.split('\n')

  return (
    <div className="relative flex h-full flex-col overflow-hidden rounded-lg border border-[var(--border-subtle)]">
      {/* Toolbar */}
      {(fileName || language) && (
        <div className="flex items-center justify-between border-b border-[var(--border-subtle)] bg-[var(--surface-base)] px-4 py-2">
          <span className="font-mono text-[11px] text-[var(--text-muted)]">
            {fileName ?? language}
          </span>
          <button
            type="button"
            onClick={handleCopy}
            className="flex min-h-[44px] items-center gap-1.5 rounded px-3 py-1 text-[11px] text-[var(--text-muted)] transition-colors hover:bg-[var(--surface-float)] hover:text-[var(--axon-primary)] focus-visible:outline-2 focus-visible:outline-[var(--focus-ring-color)] focus-visible:outline-offset-1 sm:min-h-0 sm:px-2"
          >
            {copied ? (
              <>
                <Check className="size-3" /> Copied
              </>
            ) : (
              <>
                <Copy className="size-3" /> Copy
              </>
            )}
          </button>
        </div>
      )}

      {/* Code body */}
      <div className="flex-1 overflow-auto bg-[rgba(2,4,11,0.6)]">
        <table className="w-full border-collapse font-mono text-xs">
          <tbody>
            {lines.map((line, i) => (
              <tr key={i} className="hover:bg-[var(--surface-float)]">
                <td
                  className="w-10 select-none border-r border-[var(--border-subtle)] pr-3 text-right text-[var(--text-dim)]"
                  style={{ minWidth: '2.5rem', paddingLeft: '0.5rem' }}
                >
                  {i + 1}
                </td>
                <td className="whitespace-pre pl-4 text-[var(--text-secondary)]">{line || ' '}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
