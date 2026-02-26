'use client'

import Image from 'next/image'
import type { ScreenshotFile } from '@/hooks/use-ws-messages'

interface ScreenshotRendererProps {
  files: ScreenshotFile[]
  isProcessing: boolean
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

export function ScreenshotRenderer({ files, isProcessing }: ScreenshotRendererProps) {
  if (files.length === 0 && isProcessing) {
    return (
      <div className="flex items-center gap-2 text-[var(--axon-text-muted)]">
        <span className="inline-block size-2.5 animate-spin rounded-full border-[1.5px] border-[rgba(175,215,255,0.2)] border-t-[var(--axon-accent-pink)]" />
        <span className="text-xs">Capturing screenshot...</span>
      </div>
    )
  }

  if (files.length === 0) {
    return <div className="text-sm text-[var(--axon-text-muted)]">No screenshots captured</div>
  }

  return (
    <div className="space-y-4">
      {files.map((file) => (
        <div key={file.name} className="space-y-3">
          {/* Source URL */}
          {file.url && (
            <a
              href={file.url}
              target="_blank"
              rel="noopener noreferrer"
              className="block truncate text-[length:var(--text-base)] font-medium text-[var(--axon-accent-blue-strong)] transition-colors hover:text-[var(--axon-accent-blue)] hover:underline"
            >
              {file.url}
            </a>
          )}

          {/* Screenshot image */}
          <div
            className="overflow-hidden rounded-lg border border-[rgba(255,135,175,0.1)]"
            style={{ background: 'rgba(10, 18, 35, 0.4)' }}
          >
            <Image
              src={file.serve_url ?? `/output/screenshots/${file.name}`}
              alt={`Screenshot of ${file.url || file.name}`}
              className="h-auto w-full"
              width={1600}
              height={900}
              unoptimized
            />
          </div>

          {/* Metadata bar */}
          <div className="ui-meta flex items-center gap-4">
            <span className="ui-mono">{file.name}</span>
            {file.size_bytes != null && (
              <span className="ui-mono">{formatBytes(file.size_bytes)}</span>
            )}
            <a
              href={file.serve_url ?? `/output/screenshots/${file.name}`}
              download={file.name}
              className="ml-auto text-[var(--axon-accent-blue-strong)] transition-colors hover:text-[var(--axon-accent-blue)]"
            >
              Download
            </a>
          </div>
        </div>
      ))}
    </div>
  )
}
