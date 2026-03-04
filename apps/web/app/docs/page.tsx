'use client'

import { AlertCircle, BookOpen, ChevronRight, Globe, Search } from 'lucide-react'
import dynamic from 'next/dynamic'
import { useCallback, useEffect, useMemo, useState } from 'react'
import type { DocEntry } from '@/app/api/docs/route'
import { apiFetch } from '@/lib/api-fetch'

const ContentViewer = dynamic(
  () => import('@/components/content-viewer').then((m) => ({ default: m.ContentViewer })),
  {
    ssr: false,
    loading: () => <div className="animate-pulse h-4 rounded bg-[var(--surface-elevated)]" />,
  },
)

function urlLabel(url: string): string {
  try {
    const u = new URL(url)
    const p = u.pathname.replace(/\/$/, '') || '/'
    return p.length > 56 ? `…${p.slice(-53)}` : p
  } catch {
    return url
  }
}

function groupByDomain(docs: DocEntry[]): [string, DocEntry[]][] {
  const map = new Map<string, DocEntry[]>()
  for (const d of docs) {
    const list = map.get(d.domain) ?? []
    list.push(d)
    map.set(d.domain, list)
  }
  // Sort domains by doc count descending
  return Array.from(map.entries()).sort((a, b) => b[1].length - a[1].length)
}

export default function DocsPage() {
  const [docs, setDocs] = useState<DocEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const [selected, setSelected] = useState<DocEntry | null>(null)
  const [markdown, setMarkdown] = useState<string | null>(null)
  const [loadingContent, setLoadingContent] = useState(false)
  const [contentError, setContentError] = useState<string | null>(null)

  const [query, setQuery] = useState('')
  const [expandedDomains, setExpandedDomains] = useState<Set<string>>(new Set())

  useEffect(() => {
    apiFetch('/api/docs?action=list')
      .then((r) => r.json())
      .then((data: { docs?: DocEntry[]; error?: string }) => {
        if (data.error) {
          setError(data.error)
        } else {
          const list = data.docs ?? []
          setDocs(list)
          setExpandedDomains(new Set(list.map((d) => d.domain)))
        }
      })
      .catch(() => setError('Could not read axon output directory'))
      .finally(() => setLoading(false))
  }, [])

  const handleSelect = useCallback(async (doc: DocEntry) => {
    setSelected(doc)
    setMarkdown(null)
    setContentError(null)
    setLoadingContent(true)
    try {
      const res = await apiFetch(`/api/docs?action=read&path=${encodeURIComponent(doc.relPath)}`)
      const data: { content?: string; error?: string } = await res.json()
      if (data.error) {
        setContentError(data.error)
      } else {
        setMarkdown(data.content ?? '')
      }
    } catch {
      setContentError('Network error')
    } finally {
      setLoadingContent(false)
    }
  }, [])

  const toggleDomain = useCallback((domain: string) => {
    setExpandedDomains((prev) => {
      const next = new Set(prev)
      if (next.has(domain)) next.delete(domain)
      else next.add(domain)
      return next
    })
  }, [])

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return docs
    return docs.filter((d) => d.url.toLowerCase().includes(q) || d.domain.toLowerCase().includes(q))
  }, [docs, query])

  const grouped = useMemo(() => groupByDomain(filtered), [filtered])

  return (
    <div
      className="flex h-screen flex-col overflow-hidden text-[var(--text-primary)]"
      style={{
        background:
          'radial-gradient(ellipse at 14% 10%, rgba(135,175,255,0.08), transparent 34%), radial-gradient(ellipse at 82% 16%, rgba(255,135,175,0.07), transparent 38%), linear-gradient(180deg,#02040b 0%,#030712 60%,#040a14 100%)',
      }}
    >
      {/* Header */}
      <header
        className="flex h-11 shrink-0 items-center gap-3 border-b border-[var(--border-subtle)] px-4"
        style={{ background: 'rgba(10,18,35,0.9)', backdropFilter: 'blur(16px)' }}
      >
        <div className="flex size-7 shrink-0 items-center justify-center rounded border border-[var(--border-subtle)] bg-[var(--surface-float)]">
          <BookOpen className="size-3.5 text-[var(--axon-primary)]" />
        </div>
        <div className="min-w-0 flex-1">
          <h1 className="font-display text-sm font-semibold leading-none text-[var(--text-primary)]">
            Knowledge Base
          </h1>
          <p className="mt-0.5 font-mono text-[10px] text-[var(--text-muted)]">
            Scraped and crawled pages from the omnibox
          </p>
        </div>
        {!loading && !error && (
          <span className="text-[10px] text-[var(--text-dim)]">
            {docs.length.toLocaleString()} pages
          </span>
        )}
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <aside
          className="flex w-64 shrink-0 flex-col overflow-hidden border-r border-[var(--border-subtle)]"
          style={{ background: 'rgba(10,18,35,0.5)' }}
        >
          <div className="border-b border-[var(--border-subtle)] px-2 py-2">
            <div className="flex items-center gap-2 rounded border border-[var(--border-subtle)] bg-[rgba(255,255,255,0.03)] px-2 py-1.5">
              <Search className="size-3 shrink-0 text-[var(--text-dim)]" />
              <input
                type="text"
                placeholder="Filter by URL or domain…"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                className="flex-1 bg-transparent text-[11px] text-[var(--text-secondary)] placeholder-[var(--text-dim)] outline-none"
              />
            </div>
          </div>

          <div className="flex-1 overflow-y-auto py-1">
            {loading && (
              <div className="flex items-center justify-center py-8">
                <div className="size-5 animate-spin rounded-full border-2 border-[var(--border-subtle)] border-t-[var(--axon-primary)]" />
              </div>
            )}

            {error && !loading && (
              <div className="flex items-start gap-2 px-3 py-3 text-[11px] text-[var(--axon-secondary)]">
                <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
                <span>{error}</span>
              </div>
            )}

            {!loading && !error && docs.length === 0 && (
              <div className="px-3 py-6 text-center">
                <Globe className="mx-auto mb-2 size-6 text-[var(--text-dim)]" />
                <p className="text-[11px] text-[var(--text-muted)]">No pages indexed yet</p>
                <p className="mt-1 text-[10px] text-[var(--text-dim)]">
                  Scrape or crawl URLs from the omnibox
                </p>
              </div>
            )}

            {!loading &&
              !error &&
              grouped.map(([domain, entries]) => {
                const expanded = expandedDomains.has(domain)
                return (
                  <div key={domain}>
                    <button
                      type="button"
                      onClick={() => toggleDomain(domain)}
                      className="flex w-full items-center gap-1.5 px-3 py-1.5 text-left transition-colors hover:bg-[var(--surface-float)]"
                    >
                      <ChevronRight
                        className={`size-3 shrink-0 text-[var(--text-dim)] transition-transform ${expanded ? 'rotate-90' : ''}`}
                      />
                      <Globe className="size-3 shrink-0 text-[var(--axon-primary)] opacity-60" />
                      <span className="min-w-0 flex-1 truncate text-[11px] font-medium text-[var(--text-secondary)]">
                        {domain}
                      </span>
                      <span className="shrink-0 text-[10px] text-[var(--text-dim)]">
                        {entries.length}
                      </span>
                    </button>

                    {expanded &&
                      entries.map((doc) => (
                        <button
                          key={doc.relPath}
                          type="button"
                          onClick={() => handleSelect(doc)}
                          title={doc.url}
                          className={`flex w-full items-center gap-2 py-1 pl-8 pr-3 text-left transition-colors hover:bg-[var(--surface-float)] ${
                            selected?.relPath === doc.relPath
                              ? 'bg-[rgba(175,215,255,0.06)] text-[var(--text-primary)]'
                              : 'text-[var(--text-muted)]'
                          }`}
                        >
                          <span className="min-w-0 flex-1 truncate font-mono text-[10px]">
                            {urlLabel(doc.url)}
                          </span>
                          {doc.chars > 0 && (
                            <span className="shrink-0 text-[9px] text-[var(--text-dim)]">
                              {(doc.chars / 1000).toFixed(0)}k
                            </span>
                          )}
                        </button>
                      ))}
                  </div>
                )
              })}
          </div>
        </aside>

        {/* Content pane */}
        <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
          {!selected && (
            <div className="flex h-full items-center justify-center">
              <div className="text-center">
                <BookOpen className="mx-auto mb-3 size-10 text-[var(--text-dim)]" />
                <p className="text-sm text-[var(--text-muted)]">Select a page to read it</p>
                <p className="mt-1 text-[11px] text-[var(--text-dim)]">
                  Everything scraped or crawled via the omnibox appears here
                </p>
              </div>
            </div>
          )}

          {loadingContent && (
            <div className="flex h-full items-center justify-center">
              <div className="size-6 animate-spin rounded-full border-2 border-[var(--border-subtle)] border-t-[var(--axon-primary)]" />
            </div>
          )}

          {contentError && !loadingContent && (
            <div className="m-4 flex items-center gap-2 rounded-lg border border-[var(--border-accent)] bg-[rgba(255,135,175,0.05)] px-4 py-3 text-sm text-[var(--axon-secondary)]">
              <AlertCircle className="size-4 shrink-0" />
              {contentError}
            </div>
          )}

          {selected && markdown !== null && !loadingContent && !contentError && (
            <>
              <div
                className="flex h-11 shrink-0 items-center gap-3 border-b border-[var(--border-subtle)] px-4"
                style={{ background: 'rgba(10,18,35,0.6)' }}
              >
                <Globe className="size-3.5 shrink-0 text-[var(--axon-primary)] opacity-60" />
                <a
                  href={selected.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="min-w-0 flex-1 truncate font-mono text-[11px] text-[var(--text-muted)] transition-colors hover:text-[var(--axon-primary)]"
                  title={selected.url}
                >
                  {selected.url}
                </a>
                <span className="shrink-0 font-mono text-[10px] text-[var(--text-dim)]">
                  {selected.relPath.split('/').pop()}
                </span>
              </div>
              <div className="flex-1 overflow-auto p-6">
                <div className="prose-invert mx-auto max-w-3xl">
                  <ContentViewer markdown={markdown} isProcessing={false} />
                </div>
              </div>
            </>
          )}
        </main>
      </div>
    </div>
  )
}
