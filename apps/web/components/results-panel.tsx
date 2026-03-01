'use client'

import dynamic from 'next/dynamic'
import type { ReactNode } from 'react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { ContentViewer } from '@/components/content-viewer'
import { CrawlDownloadToolbar } from '@/components/crawl-download-toolbar'
import { CrawlProgress } from '@/components/crawl-progress'
import { ExtractedSection } from '@/components/pulse/sidebar/extracted-section'
import { CardsRenderer } from '@/components/results/cards-renderer'
import { JobLifecycleRenderer } from '@/components/results/job-lifecycle-renderer'
import { RawRenderer } from '@/components/results/raw-renderer'
import { ReportRenderer } from '@/components/results/report-renderer'
import { ScreenshotRenderer } from '@/components/results/screenshot-renderer'
import { StatusRenderer } from '@/components/results/status-renderer'
import { TableRenderer } from '@/components/results/table-renderer'
import { useWsMessages } from '@/hooks/use-ws-messages'
import { AXON_COMMAND_SPECS } from '@/lib/axon-command-map'
import { normalizeResult } from '@/lib/result-normalizers'

type TabId = 'content' | 'stats' | 'recent'
const TAB_STORAGE_KEY = 'axon.web.results.active-tab'
const TAB_SCROLL_STORAGE_KEY = 'axon.web.results.tab-scroll'

const PulseWorkspace = dynamic(
  () => import('@/components/pulse/pulse-workspace').then((mod) => mod.PulseWorkspace),
  {
    ssr: false,
    loading: () => (
      <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-elevated)] p-4 text-xs text-[var(--text-dim)]">
        Loading Pulse workspace...
      </div>
    ),
  },
)

interface ResultsPanelProps {
  statsSlot?: ReactNode
}

function parseStructuredStdoutLines(stdoutLines: string[]): unknown[] {
  const parsed: unknown[] = []
  for (const line of stdoutLines) {
    const trimmed = line.trim()
    if (!trimmed || (!trimmed.startsWith('{') && !trimmed.startsWith('['))) continue
    try {
      const value = JSON.parse(trimmed) as unknown
      if (Array.isArray(value)) {
        parsed.push(...value)
      } else {
        parsed.push(value)
      }
    } catch {
      // Ignore non-JSON log lines.
    }
  }
  return parsed
}

export function selectNormalizedItems(stdoutJson: unknown[], stdoutLines: string[]): unknown[] {
  const source = stdoutJson.length > 0 ? stdoutJson : parseStructuredStdoutLines(stdoutLines)
  return source.filter((item) => !isLifecycleEntry(item))
}

function isLifecycleEntry(item: unknown): boolean {
  if (!item || typeof item !== 'object' || Array.isArray(item)) return false
  const value = item as Record<string, unknown>
  if (typeof value.job_id !== 'string') return false
  const lifecycleKeys = new Set([
    'job_id',
    'status',
    'mode',
    'phase',
    'percent',
    'processed',
    'total',
    'error_text',
  ])
  return Object.keys(value).every((key) => lifecycleKeys.has(key))
}

export function ResultsPanel({ statsSlot }: ResultsPanelProps) {
  const {
    markdownContent,
    logLines,
    errorMessage,
    recentRuns,
    isProcessing,
    hasResults,
    currentMode,
    crawlFiles,
    selectedFile,
    selectFile,
    crawlProgress,
    stdoutLines,
    stdoutJson,
    lifecycleEntries,
    commandMode,
    screenshotFiles,
    currentJobId,
    workspaceMode,
  } = useWsMessages()

  const isPulseWorkspace = workspaceMode === 'pulse'

  const [activeTab, setActiveTab] = useState<TabId>('content')
  const contentScrollRef = useRef<HTMLDivElement>(null)
  const statsScrollRef = useRef<HTMLDivElement>(null)
  const recentScrollRef = useRef<HTMLDivElement>(null)
  const tabScrollMapRef = useRef<Record<TabId, number>>({ content: 0, stats: 0, recent: 0 })
  const tabs: { id: TabId; label: string }[] = [
    { id: 'content', label: 'Content' },
    { id: 'stats', label: 'Stats' },
    { id: 'recent', label: 'Recent' },
  ]

  useEffect(() => {
    try {
      const savedTab = window.localStorage.getItem(TAB_STORAGE_KEY)
      if (savedTab === 'content' || savedTab === 'stats' || savedTab === 'recent') {
        setActiveTab(savedTab)
      }
      const raw = window.localStorage.getItem(TAB_SCROLL_STORAGE_KEY)
      if (raw) {
        const parsed = JSON.parse(raw) as Partial<Record<TabId, number>>
        tabScrollMapRef.current = {
          content: Number(parsed.content ?? 0),
          stats: Number(parsed.stats ?? 0),
          recent: Number(parsed.recent ?? 0),
        }
      }
    } catch {
      // Ignore storage failures.
    }
  }, [])

  useEffect(() => {
    const target =
      activeTab === 'content'
        ? contentScrollRef.current
        : activeTab === 'stats'
          ? statsScrollRef.current
          : recentScrollRef.current
    if (!target) return
    target.scrollTop = tabScrollMapRef.current[activeTab] ?? 0
  }, [activeTab])

  const rememberScroll = (tabId: TabId, value: number) => {
    tabScrollMapRef.current[tabId] = value
    try {
      window.localStorage.setItem(TAB_SCROLL_STORAGE_KEY, JSON.stringify(tabScrollMapRef.current))
    } catch {
      // Ignore storage failures.
    }
  }

  const switchTab = (nextTab: TabId) => {
    const activeNode =
      activeTab === 'content'
        ? contentScrollRef.current
        : activeTab === 'stats'
          ? statsScrollRef.current
          : recentScrollRef.current
    if (activeNode) rememberScroll(activeTab, activeNode.scrollTop)
    setActiveTab(nextTab)
    try {
      window.localStorage.setItem(TAB_STORAGE_KEY, nextTab)
    } catch {
      // Ignore storage failures.
    }
  }

  const effectiveCommandMode = commandMode ?? currentMode ?? null
  const isCrawlMode = currentMode === 'crawl'
  const isScreenshotMode = effectiveCommandMode === 'screenshot'
  const hasCrawlFiles = crawlFiles.length > 0
  // Crawl and scrape use file-based rendering (markdown viewer + file explorer),
  // not the renderIntent dispatch. Crawl progress arrives via dedicated
  // crawl_progress messages, not stdout_json.
  const isMarkdownMode =
    effectiveCommandMode === null ||
    effectiveCommandMode === 'scrape' ||
    effectiveCommandMode === 'crawl'

  const spec = useMemo(
    () =>
      effectiveCommandMode
        ? AXON_COMMAND_SPECS.find((s) => s.id === effectiveCommandMode)
        : undefined,
    [effectiveCommandMode],
  )

  const normalizedItems = selectNormalizedItems(stdoutJson, stdoutLines)

  const normalized = useMemo(
    () =>
      effectiveCommandMode && normalizedItems.length > 0
        ? normalizeResult(effectiveCommandMode, normalizedItems)
        : null,
    [effectiveCommandMode, normalizedItems],
  )

  const renderIntentContent = () => {
    if (spec?.renderIntent === 'job-lifecycle') {
      return (
        <JobLifecycleRenderer
          stdoutJson={lifecycleEntries.length > 0 ? lifecycleEntries : stdoutJson}
          commandMode={effectiveCommandMode}
          isProcessing={isProcessing}
          errorMessage={errorMessage}
        />
      )
    }
    if (errorMessage) {
      return (
        <div className="font-mono text-[13px] leading-relaxed text-[#ef4444]">
          <span className="mb-2 block text-sm font-bold text-[var(--axon-secondary-strong)]">
            Error
          </span>
          {errorMessage}
        </div>
      )
    }
    if (normalized && spec?.renderIntent === 'table') return <TableRenderer result={normalized} />
    if (normalized && spec?.renderIntent === 'cards') return <CardsRenderer result={normalized} />
    if (normalized && spec?.renderIntent === 'report') {
      return <ReportRenderer result={normalized} commandMode={effectiveCommandMode} />
    }
    if (normalized && spec?.renderIntent === 'status-summary') {
      return <StatusRenderer result={normalized} />
    }
    return (
      <RawRenderer stdoutJson={stdoutJson} stdoutLines={stdoutLines} isProcessing={isProcessing} />
    )
  }

  return (
    <div
      className={`overflow-hidden transition-all duration-500 ease-[cubic-bezier(0.4,0,0.2,1)] ${
        hasResults ? 'mt-2.5 max-h-[92vh] opacity-100' : 'mt-0 max-h-0 opacity-0'
      }`}
    >
      {!isPulseWorkspace && (
        <div className="mb-2.5 flex justify-end overflow-x-auto">
          <div
            className="flex w-full gap-0.5 rounded-lg border border-[var(--border-subtle)] p-[3px] sm:w-fit"
            style={{ background: 'rgba(10, 18, 35, 0.5)' }}
          >
            {tabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                onClick={() => switchTab(tab.id)}
                className={`flex-1 rounded-md px-3.5 py-2 text-center text-[11px] font-medium tracking-wide transition-all duration-200 sm:flex-none sm:py-1 ${
                  activeTab === tab.id
                    ? 'bg-[var(--surface-elevated)] font-semibold text-[var(--axon-primary)]'
                    : 'text-[var(--text-muted)] hover:bg-[var(--surface-float)] hover:text-[var(--axon-primary)]'
                }`}
              >
                {tab.label}
                {tab.id === 'content' && hasCrawlFiles && (
                  <span className="ml-1.5 text-[10px] text-[var(--text-muted)]">
                    {crawlFiles.length}
                  </span>
                )}
                {tab.id === 'stats' && logLines.length > 0 && (
                  <span className="ml-1.5 text-[10px] text-[var(--text-muted)]">
                    {logLines.length}
                  </span>
                )}
                {tab.id === 'recent' && recentRuns.length > 0 && (
                  <span className="ml-1.5 text-[10px] text-[var(--text-muted)]">
                    {recentRuns.length}
                  </span>
                )}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Content pane */}
      {(activeTab === 'content' || isPulseWorkspace) &&
        (workspaceMode === 'pulse' ? (
          <PulseWorkspace />
        ) : (
          <>
            {/* Download toolbar — visible after crawl completes */}
            {hasCrawlFiles && currentJobId && !isProcessing && (
              <div className="mb-2 flex justify-end">
                <CrawlDownloadToolbar jobId={currentJobId} fileCount={crawlFiles.length} />
              </div>
            )}
            <div
              ref={contentScrollRef}
              onScroll={() => rememberScroll('content', contentScrollRef.current?.scrollTop ?? 0)}
              className="flex max-h-[76vh] overflow-hidden rounded-[10px] border border-[var(--border-subtle)]"
              style={{ background: 'rgba(3, 7, 18, 0.42)' }}
            >
              {isScreenshotMode ? (
                <div className="flex-1 overflow-y-auto p-2 text-sm leading-[1.65] text-[var(--text-secondary)] sm:p-3 md:p-4">
                  {errorMessage ? (
                    <div className="font-mono text-[13px] leading-relaxed text-[#ef4444]">
                      <span className="mb-2 block text-sm font-bold text-[var(--axon-secondary-strong)]">
                        Error
                      </span>
                      {errorMessage}
                    </div>
                  ) : (
                    <ScreenshotRenderer files={screenshotFiles} isProcessing={isProcessing} />
                  )}
                </div>
              ) : isMarkdownMode ? (
                <>
                  {/* Crawl file list — reuses the sidebar's ExtractedSection */}
                  {hasCrawlFiles && (
                    <aside
                      className="hidden w-64 shrink-0 border-r border-[var(--border-subtle)] md:flex md:flex-col"
                      style={{ background: 'var(--surface-base)' }}
                    >
                      <ExtractedSection
                        files={crawlFiles}
                        selectedFile={selectedFile}
                        onSelectFile={selectFile}
                        jobId={currentJobId}
                      />
                    </aside>
                  )}
                  {/* Main content area */}
                  <div className="flex-1 overflow-y-auto p-3 text-sm leading-[1.75] text-[var(--text-secondary)] sm:p-4 md:p-6">
                    {/* Crawl progress bar */}
                    {isCrawlMode && isProcessing && (
                      <CrawlProgress progress={crawlProgress} isProcessing={isProcessing} />
                    )}

                    <ContentViewer
                      markdown={markdownContent}
                      isProcessing={isProcessing}
                      errorMessage={errorMessage}
                    />
                  </div>
                </>
              ) : (
                <div className="flex-1 overflow-y-auto p-3 text-sm leading-[1.75] text-[var(--text-secondary)] sm:p-4 md:p-6">
                  {renderIntentContent()}
                </div>
              )}
            </div>
          </>
        ))}

      {/* Stats pane — CLI log output + Docker stats */}
      {activeTab === 'stats' && (
        <div
          ref={statsScrollRef}
          onScroll={() => rememberScroll('stats', statsScrollRef.current?.scrollTop ?? 0)}
          className="max-h-[72vh] space-y-4 overflow-y-auto rounded-[10px] border border-[var(--border-subtle)] p-4"
          style={{ background: 'rgba(3, 7, 18, 0.42)' }}
        >
          {logLines.length > 0 && <LogViewer lines={logLines} />}
          <div className="font-mono text-xs">
            {statsSlot || (
              <div className="flex h-32 items-center justify-center text-sm text-[var(--text-muted)]">
                No stats available
              </div>
            )}
          </div>
        </div>
      )}

      {/* Recent pane */}
      {activeTab === 'recent' && (
        <div
          ref={recentScrollRef}
          onScroll={() => rememberScroll('recent', recentScrollRef.current?.scrollTop ?? 0)}
          className="max-h-[72vh] overflow-y-auto font-mono text-xs"
        >
          {recentRuns.length === 0 ? (
            <div className="flex h-32 items-center justify-center text-sm text-[var(--text-muted)]">
              No recent runs
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full border-collapse">
                <thead>
                  <tr className="text-[10px] uppercase tracking-wider text-[var(--text-muted)]">
                    <th className="w-5 border-b border-[var(--border-standard)] pb-2 text-center" />
                    <th className="border-b border-[var(--border-standard)] pb-2 text-left">
                      Mode
                    </th>
                    <th className="border-b border-[var(--border-standard)] pb-2 text-left">
                      Target
                    </th>
                    <th className="border-b border-[var(--border-standard)] pb-2 text-right">
                      Duration
                    </th>
                    <th className="border-b border-[var(--border-standard)] pb-2 text-right">
                      Lines
                    </th>
                    <th className="border-b border-[var(--border-standard)] pb-2 text-right">
                      Time
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {recentRuns.map((run) => (
                    <tr
                      key={run.id}
                      className="border-b border-[var(--border-subtle)] hover:bg-[var(--surface-float)]"
                    >
                      <td className="py-2 text-center">
                        <span
                          className={`inline-block size-[7px] rounded-full ${
                            run.status === 'done'
                              ? 'bg-[var(--axon-success)] shadow-[0_0_6px_rgba(74,222,128,0.4)]'
                              : 'bg-[var(--axon-primary)] shadow-[0_0_6px_rgba(135,175,255,0.4)]'
                          }`}
                        />
                      </td>
                      <td className="py-2 font-medium text-[var(--axon-primary)]">{run.mode}</td>
                      <td className="max-w-[260px] truncate py-2 text-[var(--text-muted)]">
                        {run.target}
                      </td>
                      <td className="py-2 text-right tabular-nums text-[var(--axon-primary)]">
                        {run.duration}
                      </td>
                      <td className="py-2 text-right tabular-nums text-[var(--text-muted)]">
                        {run.lines}
                      </td>
                      <td className="py-2 text-right text-[12px] text-[var(--text-dim)]">
                        {run.time}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function LogViewer({ lines }: { lines: { content: string; timestamp: number }[] }) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const lineCount = lines.length

  useEffect(() => {
    if (lineCount > 0 && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [lineCount])

  return (
    <div
      ref={scrollRef}
      className="max-h-[200px] overflow-y-auto rounded-lg border border-[var(--border-subtle)] p-3"
      style={{ background: 'rgba(10, 18, 35, 0.4)' }}
    >
      <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-dim)]">
        Command Log
      </div>
      {lines.map((line, i) => (
        <div key={i} className="font-mono text-[12px] leading-relaxed text-[var(--text-muted)]">
          {line.content}
        </div>
      ))}
    </div>
  )
}
