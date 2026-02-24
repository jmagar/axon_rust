'use client'

import type { CrawlProgress as CrawlProgressData } from '@/hooks/use-ws-messages'

interface CrawlProgressProps {
  progress: CrawlProgressData | null
  isProcessing: boolean
}

function phaseLabel(phase: string): string {
  switch (phase) {
    case 'crawling':
      return 'Crawling...'
    case 'sitemap':
    case 'sitemap_backfill':
      return 'Sitemap backfill...'
    case 'complete':
      return 'Complete'
    default:
      return 'Starting...'
  }
}

export function CrawlProgress({ progress, isProcessing }: CrawlProgressProps) {
  if (!isProcessing && !progress) return null

  const pages = progress?.pages_crawled ?? 0
  const discovered = progress?.pages_discovered ?? 0
  const files = progress?.md_created ?? 0
  const thin = progress?.thin_md ?? 0
  const phase = progress?.phase ?? 'pending'

  // Determinate progress when we know the approximate total
  const pct = discovered > 0 ? Math.min((pages / discovered) * 100, 100) : 0

  return (
    <div className="mb-3 space-y-1.5">
      {/* Progress bar */}
      <div className="relative h-[3px] overflow-hidden rounded-full bg-[rgba(175,215,255,0.08)]">
        {isProcessing && discovered > 0 && pct < 100 ? (
          <div
            className="h-full rounded-full transition-all duration-700"
            style={{
              width: `${pct}%`,
              background: 'linear-gradient(90deg, #ff87af, #afd7ff)',
            }}
          />
        ) : isProcessing ? (
          <div
            className="absolute inset-0 animate-shimmer rounded-full"
            style={{
              background: 'linear-gradient(90deg, #ff87af, #afd7ff, #ff87af)',
              backgroundSize: '200% 100%',
            }}
          />
        ) : (
          <div
            className="h-full rounded-full transition-all duration-500"
            style={{
              width: '100%',
              background: 'linear-gradient(90deg, #ff87af, #afd7ff)',
            }}
          />
        )}
      </div>

      {/* Counts label */}
      <div className="flex items-center gap-2 text-[11px] text-[#8787af]">
        {isProcessing && (
          <span className="inline-block size-1.5 animate-pulse rounded-full bg-[#ff87af]" />
        )}
        <span>
          {pages > 0 ? (
            <>
              {files > 0 ? files : pages}
              {files > 0 && pages > files && <> / {pages}</>}
              {discovered > 0 && isProcessing && pages === 0 && <> / ~{discovered}</>} page
              {(files > 0 ? files : pages) !== 1 ? 's' : ''}
              {thin > 0 && <> &middot; {thin} thin</>}
              {isProcessing && <> &middot; {phaseLabel(phase)}</>}
            </>
          ) : (
            isProcessing && <>{phaseLabel(phase)}</>
          )}
        </span>
      </div>
    </div>
  )
}
