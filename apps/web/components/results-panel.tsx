'use client'

import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

interface OutputLine {
  type: 'output' | 'log' | 'error'
  content: string
  parsed?: Record<string, unknown>
}

interface RecentRun {
  status: 'done' | 'failed'
  mode: string
  target: string
  duration: string
  lines: number
  time: string
}

interface ResultsPanelProps {
  lines: OutputLine[]
  recentRuns: RecentRun[]
  isProcessing: boolean
  statsSlot?: React.ReactNode
}

export type { OutputLine, RecentRun, ResultsPanelProps }

export function ResultsPanel({ lines, recentRuns, isProcessing, statsSlot }: ResultsPanelProps) {
  return (
    <Tabs defaultValue="content" className="w-full">
      <TabsList className="bg-card/50 border-border/50">
        <TabsTrigger value="content">Content</TabsTrigger>
        <TabsTrigger value="stats">Stats</TabsTrigger>
        <TabsTrigger value="recent">
          Recent
          {recentRuns.length > 0 && (
            <Badge variant="secondary" className="ml-1.5 text-[10px] px-1.5 py-0">
              {recentRuns.length}
            </Badge>
          )}
        </TabsTrigger>
      </TabsList>

      <TabsContent value="content" className="mt-3">
        <ScrollArea className="h-[60vh] rounded-lg border border-border/50 bg-card/30 p-4">
          {lines.length === 0 && !isProcessing ? (
            <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
              Run a command to see results
            </div>
          ) : (
            <div className="space-y-2 font-mono text-sm">
              {lines.map((line, i) => (
                <OutputLineRenderer key={i} line={line} />
              ))}
              {isProcessing && (
                <div className="flex items-center gap-2 text-muted-foreground">
                  <span className="size-1.5 rounded-full bg-primary animate-pulse" />
                  <span className="text-xs">Receiving...</span>
                </div>
              )}
            </div>
          )}
        </ScrollArea>
      </TabsContent>

      <TabsContent value="stats" className="mt-3">
        <ScrollArea className="h-[60vh] rounded-lg border border-border/50 bg-card/30 p-4">
          {statsSlot || (
            <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
              No stats available
            </div>
          )}
        </ScrollArea>
      </TabsContent>

      <TabsContent value="recent" className="mt-3">
        <ScrollArea className="h-[60vh] rounded-lg border border-border/50 bg-card/30 p-4">
          {recentRuns.length === 0 ? (
            <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
              No recent runs
            </div>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="text-xs text-muted-foreground border-b border-border/50">
                  <th className="pb-2 text-left w-8" />
                  <th className="pb-2 text-left">Mode</th>
                  <th className="pb-2 text-left">Target</th>
                  <th className="pb-2 text-right">Duration</th>
                  <th className="pb-2 text-right">Lines</th>
                  <th className="pb-2 text-right">Time</th>
                </tr>
              </thead>
              <tbody>
                {recentRuns.map((run, i) => (
                  <tr key={i} className="border-b border-border/30">
                    <td className="py-1.5">
                      <span
                        className={`inline-block size-2 rounded-full ${
                          run.status === 'done' ? 'bg-emerald-400' : 'bg-red-400'
                        }`}
                      />
                    </td>
                    <td className="py-1.5 font-medium">{run.mode}</td>
                    <td className="py-1.5 text-muted-foreground truncate max-w-[200px]">
                      {run.target}
                    </td>
                    <td className="py-1.5 text-right text-muted-foreground font-mono">
                      {run.duration}
                    </td>
                    <td className="py-1.5 text-right text-muted-foreground">{run.lines}</td>
                    <td className="py-1.5 text-right text-muted-foreground">{run.time}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </ScrollArea>
      </TabsContent>
    </Tabs>
  )
}

function OutputLineRenderer({ line }: { line: OutputLine }) {
  if (line.type === 'log') {
    return <div className="text-xs text-muted-foreground/70 italic">{line.content}</div>
  }

  if (line.type === 'error') {
    return (
      <div className="text-destructive border border-destructive/20 rounded-md p-3">
        <strong>Error:</strong> {line.content}
      </div>
    )
  }

  // Try to render parsed JSON content
  if (line.parsed) {
    const obj = line.parsed

    // markdown content (scrape output)
    if (typeof obj.markdown === 'string') {
      return (
        <div className="prose prose-invert prose-sm max-w-none">
          {obj.title ? <h2 className="text-foreground">{String(obj.title)}</h2> : null}
          {obj.url ? (
            <a
              href={String(obj.url)}
              target="_blank"
              rel="noopener noreferrer"
              className="text-primary text-xs"
            >
              {String(obj.url)}
            </a>
          ) : null}
          <div dangerouslySetInnerHTML={{ __html: simpleMarkdownToHtml(String(obj.markdown)) }} />
        </div>
      )
    }

    // answer content (ask output)
    if (typeof obj.answer === 'string') {
      return (
        <div className="prose prose-invert prose-sm max-w-none">
          {obj.query ? (
            <div className="text-muted-foreground mb-2">
              <strong>Q:</strong> {String(obj.query)}
            </div>
          ) : null}
          <div dangerouslySetInnerHTML={{ __html: simpleMarkdownToHtml(String(obj.answer)) }} />
        </div>
      )
    }

    // ranked result (query/retrieve output)
    if (obj.rank !== undefined && typeof obj.snippet === 'string') {
      return (
        <div className="rounded-lg border border-border/50 bg-card/50 p-3 space-y-1">
          <div className="flex items-center gap-2">
            <span className="text-xs font-bold text-primary">#{String(obj.rank)}</span>
            {obj.score != null ? (
              <span className="text-xs text-muted-foreground">{Number(obj.score).toFixed(4)}</span>
            ) : null}
            {obj.url ? (
              <a
                href={String(obj.url)}
                target="_blank"
                rel="noopener noreferrer"
                className="text-xs text-primary truncate"
              >
                {String(obj.url)}
              </a>
            ) : null}
          </div>
          <div
            className="text-sm"
            dangerouslySetInnerHTML={{ __html: simpleMarkdownToHtml(String(obj.snippet)) }}
          />
        </div>
      )
    }

    // Generic JSON — render as key-value
    return (
      <div className="rounded-lg border border-border/50 bg-card/50 p-3">
        <JsonRenderer data={obj} />
      </div>
    )
  }

  // Plain text
  return <div className="text-foreground/90">{line.content}</div>
}

function JsonRenderer({ data, depth = 0 }: { data: unknown; depth?: number }) {
  if (data === null || data === undefined)
    return <span className="text-muted-foreground">&mdash;</span>
  if (typeof data === 'string') {
    if (data.startsWith('http://') || data.startsWith('https://')) {
      return (
        <a href={data} target="_blank" rel="noopener noreferrer" className="text-primary">
          {data}
        </a>
      )
    }
    return <span>{data}</span>
  }
  if (typeof data === 'number') return <span className="text-primary">{data}</span>
  if (typeof data === 'boolean') return <span className="text-primary">{data ? 'yes' : 'no'}</span>

  if (Array.isArray(data)) {
    if (data.length === 0) return <span className="text-muted-foreground">none</span>
    return (
      <div className="space-y-1">
        {data.map((item, i) => (
          <div key={i}>
            <JsonRenderer data={item} depth={depth + 1} />
          </div>
        ))}
      </div>
    )
  }

  if (typeof data === 'object') {
    const entries = Object.entries(data as Record<string, unknown>)
    if (entries.length === 0) return <span className="text-muted-foreground">&mdash;</span>
    return (
      <div className="space-y-0.5">
        {entries.map(([key, val]) => (
          <div key={key} className="flex gap-2">
            <span className="text-muted-foreground min-w-[80px] shrink-0">
              {key.replace(/_/g, ' ')}
            </span>
            <span className="text-foreground">
              <JsonRenderer data={val} depth={depth + 1} />
            </span>
          </div>
        ))}
      </div>
    )
  }

  return <span>{String(data)}</span>
}

/** Minimal markdown-to-HTML for inline rendering in output lines. */
function simpleMarkdownToHtml(md: string): string {
  let html = ''
  const lines = md.split('\n')
  let i = 0
  let inList = false
  let listType = ''

  function esc(s: string) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  }

  function inline(s: string) {
    let result = s
    result = result.replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>')
    result = result.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    result = result.replace(/\*(.+?)\*/g, '<em>$1</em>')
    result = result.replace(
      /`([^`]+)`/g,
      '<code class="bg-muted px-1 py-0.5 rounded text-xs">$1</code>',
    )
    result = result.replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      '<a href="$2" target="_blank" class="text-primary underline">$1</a>',
    )
    return result
  }

  function closeList() {
    if (inList) {
      html += listType === 'ul' ? '</ul>\n' : '</ol>\n'
      inList = false
    }
  }

  while (i < lines.length) {
    const line = lines[i]

    // Code blocks
    if (line.trimStart().startsWith('```')) {
      closeList()
      i++
      let code = ''
      while (i < lines.length && !lines[i].trimStart().startsWith('```')) {
        code += `${esc(lines[i])}\n`
        i++
      }
      i++
      html += `<pre class="bg-muted/50 rounded-md p-3 overflow-x-auto"><code>${code}</code></pre>\n`
      continue
    }

    if (line.trim() === '') {
      closeList()
      i++
      continue
    }

    // Headers
    const hMatch = line.match(/^(#{1,6})\s+(.+)/)
    if (hMatch) {
      closeList()
      const level = hMatch[1].length
      html += `<h${level} class="font-semibold mt-3 mb-1">${inline(hMatch[2])}</h${level}>\n`
      i++
      continue
    }

    // HR
    if (/^(-{3,}|\*{3,}|_{3,})\s*$/.test(line.trim())) {
      closeList()
      html += '<hr class="border-border/50 my-3">\n'
      i++
      continue
    }

    // Table
    if (line.includes('|') && i + 1 < lines.length && /^\s*\|?\s*[-:]+/.test(lines[i + 1])) {
      closeList()
      const headers = line
        .split('|')
        .map((c) => c.trim())
        .filter(Boolean)
      i += 2
      html += '<table class="w-full text-sm border-collapse"><tr>'
      for (const h of headers)
        html += `<th class="border-b border-border/50 pb-1 text-left">${inline(h)}</th>`
      html += '</tr>\n'
      while (i < lines.length && lines[i].includes('|') && lines[i].trim()) {
        const cells = lines[i]
          .split('|')
          .map((c) => c.trim())
          .filter(Boolean)
        html += '<tr>'
        for (const c of cells) html += `<td class="py-1 pr-3">${inline(c)}</td>`
        html += '</tr>\n'
        i++
      }
      html += '</table>\n'
      continue
    }

    // Blockquote
    if (line.trimStart().startsWith('>')) {
      closeList()
      let bq = ''
      while (i < lines.length && lines[i].trimStart().startsWith('>')) {
        bq += `${lines[i].replace(/^\s*>\s?/, '')} `
        i++
      }
      html += `<blockquote class="border-l-2 border-primary/50 pl-3 text-muted-foreground italic">${inline(bq.trim())}</blockquote>\n`
      continue
    }

    // Unordered list
    if (/^\s*[-*+]\s/.test(line)) {
      if (!inList || listType !== 'ul') {
        closeList()
        html += '<ul class="list-disc pl-5 space-y-0.5">\n'
        inList = true
        listType = 'ul'
      }
      html += `<li>${inline(line.replace(/^\s*[-*+]\s/, ''))}</li>\n`
      i++
      continue
    }

    // Ordered list
    if (/^\s*\d+\.\s/.test(line)) {
      if (!inList || listType !== 'ol') {
        closeList()
        html += '<ol class="list-decimal pl-5 space-y-0.5">\n'
        inList = true
        listType = 'ol'
      }
      html += `<li>${inline(line.replace(/^\s*\d+\.\s/, ''))}</li>\n`
      i++
      continue
    }

    // Paragraph — collect consecutive non-special lines
    closeList()
    let para = ''
    while (
      i < lines.length &&
      lines[i].trim() !== '' &&
      !/^#{1,6}\s/.test(lines[i]) &&
      !/^\s*[-*+]\s/.test(lines[i]) &&
      !/^\s*\d+\.\s/.test(lines[i]) &&
      !lines[i].trimStart().startsWith('```') &&
      !lines[i].trimStart().startsWith('>')
    ) {
      para += `${lines[i]} `
      i++
    }
    if (para.trim()) html += `<p class="mb-2">${inline(para.trim())}</p>\n`
  }
  closeList()
  return html
}
