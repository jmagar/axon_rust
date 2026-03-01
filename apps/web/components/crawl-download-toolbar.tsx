'use client'

import { archiveZipUrl, packMdUrl, packXmlUrl } from '@/lib/download-urls'

interface CrawlDownloadToolbarProps {
  jobId: string
  fileCount: number
  disabled?: boolean
}

function DownloadButton({
  href,
  label,
  icon,
  disabled,
}: {
  href: string
  label: string
  icon: string
  disabled?: boolean
}) {
  if (disabled) {
    return (
      <span
        className="inline-flex cursor-not-allowed items-center gap-1.5 rounded-md border border-[var(--border-subtle)] px-2.5 py-1 text-[10px] font-medium text-[var(--text-dim)] opacity-50"
        style={{ background: 'rgba(10, 18, 35, 0.4)' }}
      >
        <svg
          aria-hidden="true"
          focusable="false"
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={1.5}
          strokeLinecap="round"
          strokeLinejoin="round"
          className="size-3"
        >
          <path d={icon} />
        </svg>
        {label}
      </span>
    )
  }

  return (
    <a
      href={href}
      download
      className="inline-flex items-center gap-1.5 rounded-md border border-[var(--border-subtle)] px-2.5 py-1 text-[10px] font-medium text-[var(--axon-secondary)] transition-all hover:border-[rgba(175,215,255,0.3)] hover:text-[var(--axon-primary-strong)]"
      style={{ background: 'rgba(10, 18, 35, 0.4)' }}
    >
      <svg
        aria-hidden="true"
        focusable="false"
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
        className="size-3"
      >
        <path d={icon} />
      </svg>
      {label}
    </a>
  )
}

const DOWNLOAD_ICON = 'M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4'
const ARCHIVE_ICON = 'M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4'

export function CrawlDownloadToolbar({ jobId, fileCount, disabled }: CrawlDownloadToolbarProps) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-[10px] text-[var(--text-dim)]">{fileCount} pages</span>
      <div className="h-3 w-px bg-[var(--border-subtle)]" />
      <DownloadButton
        href={packMdUrl(jobId)}
        label="Pack (MD)"
        icon={DOWNLOAD_ICON}
        disabled={disabled}
      />
      <DownloadButton
        href={packXmlUrl(jobId)}
        label="Pack (XML)"
        icon={DOWNLOAD_ICON}
        disabled={disabled}
      />
      <DownloadButton
        href={archiveZipUrl(jobId)}
        label="ZIP"
        icon={ARCHIVE_ICON}
        disabled={disabled}
      />
    </div>
  )
}
