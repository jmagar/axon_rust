import { describe, expect, it } from 'vitest'
import {
  isUrlLikeToken,
  normalizeUrlInput,
  shouldPreservePulseWorkspaceForMode,
  shouldRunCommandForInput,
} from '@/components/omnibox/utils'

describe('isUrlLikeToken', () => {
  it('returns true for http URLs', () => {
    expect(isUrlLikeToken('http://example.com')).toBe(true)
  })

  it('returns true for https URLs', () => {
    expect(isUrlLikeToken('https://docs.astral.sh/ruff')).toBe(true)
  })

  it('returns true for case-insensitive scheme', () => {
    expect(isUrlLikeToken('HTTPS://Example.COM')).toBe(true)
  })

  it('returns true for bare domain', () => {
    expect(isUrlLikeToken('docs.astral.sh')).toBe(true)
  })

  it('returns true for bare domain with path', () => {
    expect(isUrlLikeToken('example.com/page')).toBe(true)
  })

  it('returns true for bare domain with port', () => {
    expect(isUrlLikeToken('example.com:8080')).toBe(true)
  })

  it('returns true for bare domain with query', () => {
    expect(isUrlLikeToken('example.com?q=test')).toBe(true)
  })

  it('returns true for bare domain with fragment', () => {
    expect(isUrlLikeToken('example.com#section')).toBe(true)
  })

  it('returns false for empty string', () => {
    expect(isUrlLikeToken('')).toBe(false)
  })

  it('returns false for email-like token', () => {
    expect(isUrlLikeToken('user@example.com')).toBe(false)
  })

  it('returns false for plain word', () => {
    expect(isUrlLikeToken('hello')).toBe(false)
  })

  it('returns false for single-label hostname', () => {
    expect(isUrlLikeToken('localhost')).toBe(false)
  })

  it('returns false for TLD too short', () => {
    expect(isUrlLikeToken('example.x')).toBe(false)
  })

  it('returns true for two-letter TLD', () => {
    expect(isUrlLikeToken('example.io')).toBe(true)
  })

  it('returns true for subdomain chains', () => {
    expect(isUrlLikeToken('a.b.c.example.com')).toBe(true)
  })
})

describe('normalizeUrlInput', () => {
  it('prepends https:// to bare domain', () => {
    expect(normalizeUrlInput('example.com')).toBe('https://example.com')
  })

  it('does not prepend if already has scheme', () => {
    expect(normalizeUrlInput('https://example.com')).toBe('https://example.com')
  })

  it('does not prepend for http scheme', () => {
    expect(normalizeUrlInput('http://example.com')).toBe('http://example.com')
  })

  it('returns trimmed input for non-URL text', () => {
    expect(normalizeUrlInput('  hello world  ')).toBe('hello world')
  })

  it('returns empty string for whitespace-only input', () => {
    expect(normalizeUrlInput('   ')).toBe('')
  })

  it('does not prepend when input has multiple tokens', () => {
    expect(normalizeUrlInput('example.com extra stuff')).toBe('example.com extra stuff')
  })

  it('does not prepend for single non-URL token', () => {
    expect(normalizeUrlInput('hello')).toBe('hello')
  })
})

describe('shouldRunCommandForInput', () => {
  it('returns true for non-URL mode with any input', () => {
    expect(shouldRunCommandForInput('query', 'some search text')).toBe(true)
  })

  it('returns true for URL mode with valid URL input', () => {
    expect(shouldRunCommandForInput('scrape', 'https://example.com')).toBe(true)
  })

  it('returns true for URL mode with bare domain input', () => {
    expect(shouldRunCommandForInput('crawl', 'docs.astral.sh')).toBe(true)
  })

  it('returns false for URL mode with non-URL input', () => {
    expect(shouldRunCommandForInput('scrape', 'just some text')).toBe(false)
  })

  it('allows empty input for NO_INPUT_MODES (stats)', () => {
    expect(shouldRunCommandForInput('stats', '')).toBe(true)
  })

  it('allows empty input for NO_INPUT_MODES (doctor)', () => {
    expect(shouldRunCommandForInput('doctor', '')).toBe(true)
  })

  it('rejects empty input for modes not in NO_INPUT_MODES', () => {
    expect(shouldRunCommandForInput('query', '')).toBe(false)
  })

  it('rejects empty input for URL modes', () => {
    expect(shouldRunCommandForInput('scrape', '')).toBe(false)
  })

  it('handles whitespace-only input as empty', () => {
    expect(shouldRunCommandForInput('scrape', '   ')).toBe(false)
  })

  it('checks first token only for URL modes', () => {
    expect(shouldRunCommandForInput('crawl', 'https://example.com --depth 3')).toBe(true)
  })
})

describe('shouldPreservePulseWorkspaceForMode', () => {
  it('returns true for pulse + scrape', () => {
    expect(shouldPreservePulseWorkspaceForMode('pulse', 'scrape')).toBe(true)
  })

  it('returns true for pulse + crawl', () => {
    expect(shouldPreservePulseWorkspaceForMode('pulse', 'crawl')).toBe(true)
  })

  it('returns true for pulse + extract', () => {
    expect(shouldPreservePulseWorkspaceForMode('pulse', 'extract')).toBe(true)
  })

  it('returns false for pulse + query', () => {
    expect(shouldPreservePulseWorkspaceForMode('pulse', 'query')).toBe(false)
  })

  it('returns false for non-pulse workspace', () => {
    expect(shouldPreservePulseWorkspaceForMode('dashboard', 'scrape')).toBe(false)
  })

  it('returns false for null workspace', () => {
    expect(shouldPreservePulseWorkspaceForMode(null, 'scrape')).toBe(false)
  })
})
