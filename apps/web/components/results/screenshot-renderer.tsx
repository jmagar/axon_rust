'use client'

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
      <div className="flex items-center gap-2 text-[#8787af]">
        <span className="inline-block size-2.5 animate-spin rounded-full border-[1.5px] border-[rgba(255,135,175,0.2)] border-t-[#ff87af]" />
        <span className="text-xs">Capturing screenshot...</span>
      </div>
    )
  }

  if (files.length === 0) {
    return <div className="text-sm text-[#8787af]">No screenshots captured</div>
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
              className="block truncate text-[13px] font-medium text-[#87afff] transition-colors hover:text-[#afd7ff] hover:underline"
            >
              {file.url}
            </a>
          )}

          {/* Screenshot image */}
          <div
            className="overflow-hidden rounded-lg border border-[rgba(175,215,255,0.1)]"
            style={{ background: 'rgba(10, 18, 35, 0.4)' }}
          >
            <img
              src={file.serve_url ?? `/output/screenshots/${file.name}`}
              alt={`Screenshot of ${file.url || file.name}`}
              className="w-full"
              loading="lazy"
            />
          </div>

          {/* Metadata bar */}
          <div className="flex items-center gap-4 text-[11px] text-[#5f6b7a]">
            <span className="font-mono">{file.name}</span>
            {file.size_bytes != null && (
              <span className="font-mono">{formatBytes(file.size_bytes)}</span>
            )}
            <a
              href={file.serve_url ?? `/output/screenshots/${file.name}`}
              download={file.name}
              className="ml-auto text-[#87afff] transition-colors hover:text-[#afd7ff]"
            >
              Download
            </a>
          </div>
        </div>
      ))}
    </div>
  )
}
