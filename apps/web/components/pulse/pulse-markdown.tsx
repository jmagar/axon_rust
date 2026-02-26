'use client'

import type { ReactNode } from 'react'

// ── Inline formatting ──────────────────────────────────────────────────────────

const INLINE_RE = /(`[^`\n]+`|\*\*[^*\n]+\*\*|\*[^*\n]+\*|~~[^~\n]+~~|\[[^\]]+\]\([^)]+\))/g

function renderInline(text: string): ReactNode[] {
  const parts = text.split(INLINE_RE)
  return parts.map((part, i) => {
    if (part.startsWith('`') && part.endsWith('`') && part.length > 2) {
      return (
        <code
          key={i}
          className="rounded bg-[rgba(175,215,255,0.1)] px-[0.3em] py-[0.1em] font-mono text-[0.88em] text-[var(--axon-accent-pink-strong)]"
        >
          {part.slice(1, -1)}
        </code>
      )
    }
    if (part.startsWith('**') && part.endsWith('**')) {
      return (
        <strong key={i} className="font-semibold text-[var(--axon-text-primary)]">
          {part.slice(2, -2)}
        </strong>
      )
    }
    if (part.startsWith('~~') && part.endsWith('~~')) {
      return (
        <del key={i} className="text-[var(--axon-text-muted)] line-through">
          {part.slice(2, -2)}
        </del>
      )
    }
    if (part.startsWith('*') && part.endsWith('*')) {
      return <em key={i}>{part.slice(1, -1)}</em>
    }
    const linkMatch = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(part)
    if (linkMatch) {
      return (
        <a
          key={i}
          href={linkMatch[2]}
          target="_blank"
          rel="noreferrer"
          className="text-[var(--axon-accent-blue)] underline underline-offset-2 hover:text-[var(--axon-accent-blue-strong)]"
        >
          {linkMatch[1]}
        </a>
      )
    }
    return part
  })
}

// ── Code block ────────────────────────────────────────────────────────────────

function CodeBlock({ lang, code }: { lang: string; code: string }) {
  return (
    <div className="my-2 overflow-hidden rounded-lg border border-[rgba(175,215,255,0.14)] bg-[rgba(5,10,22,0.65)]">
      {lang && (
        <div className="border-b border-[rgba(175,215,255,0.1)] px-3 py-1 font-mono text-[0.68rem] tracking-widest text-[var(--axon-text-dim)] uppercase">
          {lang}
        </div>
      )}
      <pre className="overflow-x-auto p-3 font-mono text-[0.8rem] leading-[1.6] text-[var(--axon-text-secondary)]">
        <code>{code}</code>
      </pre>
    </div>
  )
}

// ── List item (supports one level of nesting) ─────────────────────────────────

type ListEntry = { text: string; depth: number }

function ListBlock({ items, ordered }: { items: ListEntry[]; ordered: boolean }) {
  const Tag = ordered ? 'ol' : 'ul'
  // Split into top-level groups with their nested children
  const groups: Array<{ text: string; children: ListEntry[] }> = []
  for (const item of items) {
    if (item.depth === 0) {
      groups.push({ text: item.text, children: [] })
    } else {
      const last = groups[groups.length - 1]
      if (last) last.children.push(item)
    }
  }
  return (
    <Tag
      className={`my-1.5 space-y-0.5 pl-4 ${ordered ? 'list-decimal' : 'list-disc'} text-[var(--axon-text-secondary)]`}
    >
      {groups.map((g, i) => (
        <li key={i} className="text-[length:var(--text-md)] leading-[var(--leading-copy)]">
          {renderInline(g.text)}
          {g.children.length > 0 && (
            <ul className="mt-0.5 space-y-0.5 pl-4 list-[circle] text-[var(--axon-text-secondary)]">
              {g.children.map((c, j) => (
                <li key={j} className="text-[length:var(--text-md)] leading-[var(--leading-copy)]">
                  {renderInline(c.text)}
                </li>
              ))}
            </ul>
          )}
        </li>
      ))}
    </Tag>
  )
}

// ── Block-level line renderer ─────────────────────────────────────────────────

function renderLines(lines: string[]): ReactNode[] {
  const nodes: ReactNode[] = []
  let listItems: ListEntry[] = []
  let listOrdered = false
  let blockquoteLines: string[] = []

  function flushList() {
    if (!listItems.length) return
    nodes.push(<ListBlock key={`list-${nodes.length}`} items={listItems} ordered={listOrdered} />)
    listItems = []
  }

  function flushBlockquote() {
    if (!blockquoteLines.length) return
    nodes.push(
      <blockquote
        key={`bq-${nodes.length}`}
        className="my-1.5 border-l-2 border-[rgba(175,215,255,0.3)] pl-3 text-[var(--axon-text-muted)] italic"
      >
        {blockquoteLines.map((line, i) => (
          <p key={i} className="text-[length:var(--text-md)] leading-[var(--leading-copy)]">
            {renderInline(line)}
          </p>
        ))}
      </blockquote>,
    )
    blockquoteLines = []
  }

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? ''

    // Headings
    const heading = /^(#{1,4})\s+(.+)$/.exec(line)
    if (heading) {
      flushList()
      flushBlockquote()
      const level = heading[1].length
      const text = heading[2]
      const cls =
        level === 1
          ? 'mt-3 mb-1 text-[length:var(--text-base)] font-semibold text-[var(--axon-text-primary)]'
          : level === 2
            ? 'mt-2.5 mb-1 text-[length:var(--text-md)] font-semibold text-[var(--axon-text-primary)]'
            : 'mt-2 mb-0.5 text-[length:var(--text-md)] font-medium text-[var(--axon-text-secondary)]'
      nodes.push(
        <p key={`h-${i}`} className={cls}>
          {renderInline(text)}
        </p>,
      )
      continue
    }

    // Blockquote
    const bqMatch = /^>\s?(.*)$/.exec(line)
    if (bqMatch) {
      flushList()
      blockquoteLines.push(bqMatch[1])
      continue
    }
    flushBlockquote()

    // Unordered list (supports 0–2 spaces indent for nesting)
    const ulItem = /^(\s{0,3})[-*]\s+(.+)$/.exec(line)
    if (ulItem) {
      if (listItems.length > 0 && listOrdered) flushList()
      listOrdered = false
      const depth = ulItem[1].length >= 2 ? 1 : 0
      listItems.push({ text: ulItem[2], depth })
      continue
    }

    // Ordered list
    const olItem = /^(\s{0,3})\d+\.\s+(.+)$/.exec(line)
    if (olItem) {
      if (listItems.length > 0 && !listOrdered) flushList()
      listOrdered = true
      const depth = olItem[1].length >= 2 ? 1 : 0
      listItems.push({ text: olItem[2], depth })
      continue
    }

    flushList()

    // Horizontal rule
    if (/^---+$/.test(line.trim())) {
      nodes.push(<hr key={`hr-${i}`} className="my-2 border-[rgba(255,255,255,0.08)]" />)
      continue
    }

    // Empty line → spacer
    if (!line.trim()) {
      if (nodes.length > 0) {
        nodes.push(<div key={`sp-${i}`} className="h-1.5" />)
      }
      continue
    }

    // Normal paragraph
    nodes.push(
      <p
        key={`p-${i}`}
        className="text-[length:var(--text-md)] leading-[var(--leading-copy)] text-[var(--axon-text-secondary)]"
      >
        {renderInline(line)}
      </p>,
    )
  }

  flushList()
  flushBlockquote()
  return nodes
}

// ── Public component ──────────────────────────────────────────────────────────

export function PulseMarkdown({ content }: { content: string }) {
  // Split on fenced code blocks first, preserving them as separate segments
  const segments = content.split(/(```[\w]*\n[\s\S]*?\n```)/g)

  return (
    <div className="space-y-0.5">
      {segments.map((seg, i) => {
        const fenced = /^```([\w]*)\n([\s\S]*?)\n```$/.exec(seg)
        if (fenced) {
          return <CodeBlock key={i} lang={fenced[1]} code={fenced[2]} />
        }
        return <div key={i}>{renderLines(seg.split('\n'))}</div>
      })}
    </div>
  )
}
