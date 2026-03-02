import { FileUp } from 'lucide-react'

import type { TFileElement } from 'platejs'
import type { SlateElementProps } from 'platejs/static'
import { SlateElement } from 'platejs/static'

const SAFE_FILE_PROTOCOLS = new Set(['blob:', 'http:', 'https:', 'mailto:', 'tel:'])

function getSafeFileHref(url: unknown): string | undefined {
  if (typeof url !== 'string') return undefined

  const trimmed = url.trim()
  if (!trimmed) return undefined
  if (trimmed.startsWith('/') || trimmed.startsWith('./') || trimmed.startsWith('../')) {
    return trimmed
  }

  try {
    const parsed = new URL(trimmed)
    return SAFE_FILE_PROTOCOLS.has(parsed.protocol) ? trimmed : undefined
  } catch {
    // URL parsing failed — if there's no colon it's a bare relative path (e.g. "attachments/report.pdf")
    return trimmed.includes(':') ? undefined : trimmed
  }
}

export function FileElementStatic(props: SlateElementProps<TFileElement>) {
  const { name, url } = props.element
  const safeUrl = getSafeFileHref(url)

  return (
    <SlateElement className="my-px rounded-sm" {...props}>
      <a
        className="group relative m-0 flex cursor-pointer items-center rounded px-0.5 py-[3px] hover:bg-muted"
        contentEditable={false}
        download={name}
        href={safeUrl}
        rel="noopener noreferrer"
        role="button"
        target="_blank"
      >
        <div className="flex items-center gap-1 p-1">
          <FileUp className="size-5" />
          <div>{name}</div>
        </div>
      </a>
      {props.children}
    </SlateElement>
  )
}
