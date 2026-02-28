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
    <div className="relative flex h-full flex-col overflow-hidden rounded-lg border border-[rgba(175,215,255,0.08)]">
      {/* Toolbar */}
      {(fileName || language) && (
        <div className="flex items-center justify-between border-b border-[rgba(175,215,255,0.08)] bg-[rgba(4,10,20,0.8)] px-4 py-2">
          <span className="font-mono text-[11px] text-[rgba(175,215,255,0.5)]">
            {fileName ?? language}
          </span>
          <button
            type="button"
            onClick={handleCopy}
            className="flex items-center gap-1.5 rounded px-2 py-1 text-[11px] text-[rgba(175,215,255,0.5)] transition-colors hover:bg-[rgba(175,215,255,0.08)] hover:text-[rgba(175,215,255,0.9)]"
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
              <tr key={i} className="hover:bg-[rgba(255,255,255,0.02)]">
                <td
                  className="w-10 select-none border-r border-[rgba(255,255,255,0.04)] pr-3 text-right text-[rgba(200,210,230,0.2)]"
                  style={{ minWidth: '2.5rem', paddingLeft: '0.5rem' }}
                >
                  {i + 1}
                </td>
                <td className="whitespace-pre pl-4 text-[rgba(200,220,245,0.8)]">{line || ' '}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
