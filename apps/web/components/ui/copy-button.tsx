'use client'

import { useCallback, useState } from 'react'

interface CopyButtonProps {
  text: string
  className?: string
}

export function CopyButton({ text, className }: CopyButtonProps) {
  const [copied, setCopied] = useState(false)

  const handleCopy = useCallback(async () => {
    await navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }, [text])

  return (
    <button
      type="button"
      onClick={handleCopy}
      className={
        className ??
        'absolute right-3 top-3 z-10 flex items-center gap-1.5 rounded-md border border-[rgba(255,135,175,0.1)] px-2 py-1 text-[11px] font-medium text-[var(--axon-text-muted)] transition-all duration-200 hover:border-[rgba(255,135,175,0.2)] hover:bg-[rgba(255,135,175,0.06)] hover:text-[var(--axon-accent-blue)]'
      }
      style={{ background: 'rgba(10, 18, 35, 0.8)', backdropFilter: 'blur(4px)' }}
    >
      <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
        className="size-3"
      >
        {copied ? (
          <path d="M20 6 9 17l-5-5" />
        ) : (
          <>
            <rect x={9} y={9} width={13} height={13} rx={2} ry={2} />
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
          </>
        )}
      </svg>
      {copied ? 'Copied' : 'Copy'}
    </button>
  )
}
