'use client'

import type { ReactNode } from 'react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { ContentViewer } from '@/components/content-viewer'
import { CrawlDownloadToolbar } from '@/components/crawl-download-toolbar'
import { CrawlFileExplorer } from '@/components/crawl-file-explorer'
import { CrawlProgress } from '@/components/crawl-progress'
import { PulseWorkspace } from '@/components/pulse/pulse-workspace'
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

interface ResultsPanelProps {
  statsSlot?: ReactNode
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
    commandMode,
    screenshotFiles,
    currentJobId,
    workspaceMode,
  } = useWsMessages()

  const [activeTab, setActiveTab] = useState<TabId>('content')
  const tabs: { id: TabId; label: string }[] = [
    { id: 'content', label: 'Content' },
    { id: 'stats', label: 'Stats' },
    { id: 'recent', label: 'Recent' },
  ]

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

  const fallbackJsonItems = useMemo(() => {
    if (stdoutJson.length > 0 || stdoutLines.length === 0) return null

    const joined = stdoutLines.join('\n')
    const candidates = [joined.trim()]

    // Also try parsing from the first opening bracket to the last closing bracket.
    // This recovers when stdout has harmless prefix/suffix text around JSON.
    const firstObj = joined.indexOf('{')
    const lastObj = joined.lastIndexOf('}')
    if (firstObj >= 0 && lastObj > firstObj) {
      candidates.push(joined.slice(firstObj, lastObj + 1).trim())
    }
    const firstArr = joined.indexOf('[')
    const lastArr = joined.lastIndexOf(']')
    if (firstArr >= 0 && lastArr > firstArr) {
      candidates.push(joined.slice(firstArr, lastArr + 1).trim())
    }

    for (const candidate of candidates) {
      if (!candidate || (!candidate.startsWith('{') && !candidate.startsWith('['))) continue
      try {
        const parsed = JSON.parse(candidate)
        return [parsed]
      } catch {
        // Try next candidate
      }
    }
    return null
  }, [stdoutJson, stdoutLines])

  const normalizedItems = stdoutJson.length > 0 ? stdoutJson : (fallbackJsonItems ?? [])

  const normalized = useMemo(
    () =>
      effectiveCommandMode && normalizedItems.length > 0
        ? normalizeResult(effectiveCommandMode, normalizedItems)
        : null,
    [effectiveCommandMode, normalizedItems],
  )

  return (
    <div
      className={`overflow-hidden transition-all duration-500 ease-[cubic-bezier(0.4,0,0.2,1)] ${
        hasResults ? 'mt-4 max-h-[90vh] opacity-100' : 'mt-0 max-h-0 opacity-0'
      }`}
    >
      {/* Tab bar */}
      <div className="mb-2.5 flex justify-end overflow-x-auto">
        <div
          className="flex w-fit gap-0.5 rounded-lg border border-[rgba(175,215,255,0.1)] p-[3px]"
          style={{ background: 'rgba(10, 18, 35, 0.5)' }}
        >
          {tabs.map((tab) => (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className={`rounded-md px-3.5 py-1 text-[11px] font-medium tracking-wide transition-all duration-200 ${
                activeTab === tab.id
                  ? 'bg-[rgba(175,215,255,0.1)] font-semibold text-[#afd7ff]'
                  : 'text-[#8787af] hover:bg-[rgba(175,215,255,0.06)] hover:text-[#afd7ff]'
              }`}
            >
              {tab.label}
              {tab.id === 'content' && hasCrawlFiles && (
                <span className="ml-1.5 text-[9px] text-[#8787af]">{crawlFiles.length}</span>
              )}
              {tab.id === 'stats' && logLines.length > 0 && (
                <span className="ml-1.5 text-[9px] text-[#8787af]">{logLines.length}</span>
              )}
              {tab.id === 'recent' && recentRuns.length > 0 && (
                <span className="ml-1.5 text-[9px] text-[#8787af]">{recentRuns.length}</span>
              )}
            </button>
          ))}
        </div>
      </div>

      {/* Content pane */}
      {activeTab === 'content' &&
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
              className="flex max-h-[72vh] overflow-hidden rounded-[10px] border border-[rgba(175,215,255,0.1)]"
              style={{ background: 'rgba(3, 7, 18, 0.25)' }}
            >
              {isScreenshotMode ? (
                <div className="flex-1 overflow-y-auto p-2 text-sm leading-[1.65] text-[#dce6f0] sm:p-3 md:p-4">
                  {errorMessage ? (
                    <div className="font-mono text-[13px] leading-relaxed text-[#ef4444]">
                      <span className="mb-2 block text-sm font-bold text-[#ff87af]">Error</span>
                      {errorMessage}
                    </div>
                  ) : (
                    <ScreenshotRenderer files={screenshotFiles} isProcessing={isProcessing} />
                  )}
                </div>
              ) : isMarkdownMode ? (
                <>
                  {/* Crawl file explorer sidebar (drawer on mobile, inline on desktop) */}
                  {hasCrawlFiles && (
                    <CrawlFileExplorer
                      files={crawlFiles}
                      selectedFile={selectedFile}
                      onSelectFile={selectFile}
                      jobId={currentJobId}
                    />
                  )}

                  {/* Main content area */}
                  <div className="flex-1 overflow-y-auto p-3 text-sm leading-[1.75] text-[#dce6f0] sm:p-4 md:p-6">
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
                <div className="flex-1 overflow-y-auto p-3 text-sm leading-[1.75] text-[#dce6f0] sm:p-4 md:p-6">
                  {spec?.renderIntent === 'job-lifecycle' ? (
                    <JobLifecycleRenderer
                      stdoutJson={stdoutJson}
                      commandMode={effectiveCommandMode}
                      isProcessing={isProcessing}
                      errorMessage={errorMessage}
                    />
                  ) : errorMessage ? (
                    <div className="font-mono text-[13px] leading-relaxed text-[#ef4444]">
                      <span className="mb-2 block text-sm font-bold text-[#ff87af]">Error</span>
                      {errorMessage}
                    </div>
                  ) : normalized && spec?.renderIntent === 'table' ? (
                    <TableRenderer result={normalized} />
                  ) : normalized && spec?.renderIntent === 'cards' ? (
                    <CardsRenderer result={normalized} />
                  ) : normalized && spec?.renderIntent === 'report' ? (
                    <ReportRenderer result={normalized} commandMode={effectiveCommandMode} />
                  ) : normalized && spec?.renderIntent === 'status-summary' ? (
                    <StatusRenderer result={normalized} />
                  ) : (
                    <RawRenderer
                      stdoutJson={stdoutJson}
                      stdoutLines={stdoutLines}
                      isProcessing={isProcessing}
                    />
                  )}
                </div>
              )}
            </div>
          </>
        ))}

      {/* Stats pane — CLI log output + Docker stats */}
      {activeTab === 'stats' && (
        <div
          className="max-h-[72vh] space-y-4 overflow-y-auto rounded-[10px] border border-[rgba(175,215,255,0.1)] p-4"
          style={{ background: 'rgba(3, 7, 18, 0.25)' }}
        >
          {logLines.length > 0 && <LogViewer lines={logLines} />}
          <div className="font-mono text-xs">
            {statsSlot || (
              <div className="flex h-32 items-center justify-center text-sm text-[#8787af]">
                No stats available
              </div>
            )}
          </div>
        </div>
      )}

      {/* Recent pane */}
      {activeTab === 'recent' && (
        <div className="font-mono text-xs">
          {recentRuns.length === 0 ? (
            <div className="flex h-32 items-center justify-center text-sm text-[#8787af]">
              No recent runs
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full border-collapse">
                <thead>
                  <tr className="text-[10px] uppercase tracking-wider text-[#8787af]">
                    <th className="w-5 border-b border-[rgba(175,215,255,0.15)] pb-2 text-center" />
                    <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-left">
                      Mode
                    </th>
                    <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-left">
                      Target
                    </th>
                    <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-right">
                      Duration
                    </th>
                    <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-right">
                      Lines
                    </th>
                    <th className="border-b border-[rgba(175,215,255,0.15)] pb-2 text-right">
                      Time
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {recentRuns.map((run) => (
                    <tr
                      key={run.id}
                      className="border-b border-[rgba(175,215,255,0.05)] hover:bg-[rgba(175,215,255,0.03)]"
                    >
                      <td className="py-2 text-center">
                        <span
                          className={`inline-block size-[7px] rounded-full ${
                            run.status === 'done'
                              ? 'bg-[#4ade80] shadow-[0_0_6px_rgba(74,222,128,0.4)]'
                              : 'bg-[#ff87af] shadow-[0_0_6px_rgba(255,135,175,0.4)]'
                          }`}
                        />
                      </td>
                      <td className="py-2 font-medium text-[#afd7ff]">{run.mode}</td>
                      <td className="max-w-[260px] truncate py-2 text-[#8787af]">{run.target}</td>
                      <td className="py-2 text-right tabular-nums text-[#afd7ff]">
                        {run.duration}
                      </td>
                      <td className="py-2 text-right tabular-nums text-[#94a3b8]">{run.lines}</td>
                      <td className="py-2 text-right text-[11px] text-[#475569]">{run.time}</td>
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
      className="max-h-[200px] overflow-y-auto rounded-lg border border-[rgba(175,215,255,0.08)] p-3"
      style={{ background: 'rgba(10, 18, 35, 0.4)' }}
    >
      <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-[#5f87af]">
        Command Log
      </div>
      {lines.map((line, i) => (
        <div key={i} className="font-mono text-[11px] leading-relaxed text-[#8787af]">
          {line.content}
        </div>
      ))}
    </div>
  )
}
