import { describe, expect, it } from 'vitest'
import type { LocalDocFile } from '@/lib/omnibox'
import {
  deriveOmniboxPhase,
  extractActiveMention,
  extractMentionLabels,
  getMentionKind,
  rankFileSuggestions,
  rankModeSuggestions,
  replaceActiveMention,
} from '@/lib/omnibox'
import { MODES } from '@/lib/ws-protocol'

describe('omnibox mention parsing', () => {
  it('extracts active mention token from mode shorthand', () => {
    const token = extractActiveMention('@c')
    expect(token).toEqual({ query: 'c', start: 0, end: 2 })
  })

  it('returns null when mention is not at the end', () => {
    expect(extractActiveMention('open @readme and continue')).toBeNull()
  })

  it('classifies pure mention as mode and mixed text as file mention', () => {
    const modeToken = extractActiveMention('@crawl')
    const fileToken = extractActiveMention('summarize @docs/overview')

    expect(getMentionKind('@crawl', modeToken)).toBe('mode')
    expect(getMentionKind('summarize @docs/overview', fileToken)).toBe('file')
  })
})

describe('omnibox suggestion ranking', () => {
  it('prioritizes mode prefix matches', () => {
    const ranked = rankModeSuggestions(MODES, 'cr', 3)
    expect(ranked.length).toBeGreaterThan(0)
    expect(ranked[0]?.id).toBe('crawl')
  })

  it('returns no mode suggestions for empty query', () => {
    expect(rankModeSuggestions(MODES, '', 3)).toEqual([])
  })

  it('prioritizes exact and recent file candidates', () => {
    const files: LocalDocFile[] = [
      { id: 'a', label: 'readme', path: 'docs/readme.md', source: 'docs' },
      { id: 'b', label: 'reader-guide', path: 'docs/reader-guide.md', source: 'docs' },
      { id: 'c', label: 'read-index', path: 'docs/read-index.md', source: 'docs' },
    ]

    const exact = rankFileSuggestions(files, 'readme', {}, 3)
    expect(exact[0]?.id).toBe('a')

    const recent = rankFileSuggestions(files, 'read', { c: Date.now() }, 3)
    expect(recent[0]?.id).toBe('c')
  })
})

describe('omnibox text transforms and phases', () => {
  it('replaces only the active mention segment', () => {
    const input = 'summarize @rea'
    const token = extractActiveMention(input)
    if (!token) throw new Error('expected mention token')

    const replaced = replaceActiveMention(input, token, '@readme ')
    expect(replaced).toBe('summarize @readme ')
  })

  it('extracts unique mention labels from input', () => {
    const labels = extractMentionLabels('compare @readme with @guide and @readme')
    expect(labels).toEqual(['readme', 'guide'])
  })

  it('derives expected phase transitions', () => {
    expect(
      deriveOmniboxPhase({
        isProcessing: false,
        input: '',
        mentionKind: 'none',
        hasModeFeedback: false,
      }),
    ).toBe('idle')

    expect(
      deriveOmniboxPhase({
        isProcessing: false,
        input: '@c',
        mentionKind: 'mode',
        hasModeFeedback: false,
      }),
    ).toBe('mode-mention')

    expect(
      deriveOmniboxPhase({
        isProcessing: false,
        input: 'query @readme',
        mentionKind: 'file',
        hasModeFeedback: false,
      }),
    ).toBe('file-mention')

    expect(
      deriveOmniboxPhase({
        isProcessing: false,
        input: 'https://example.com',
        mentionKind: 'none',
        hasModeFeedback: false,
      }),
    ).toBe('ready')

    expect(
      deriveOmniboxPhase({
        isProcessing: true,
        input: 'https://example.com',
        mentionKind: 'none',
        hasModeFeedback: false,
      }),
    ).toBe('executing')

    expect(
      deriveOmniboxPhase({
        isProcessing: false,
        input: '',
        mentionKind: 'none',
        hasModeFeedback: true,
      }),
    ).toBe('mode-selected')
  })
})
