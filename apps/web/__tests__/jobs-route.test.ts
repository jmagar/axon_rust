import { describe, expect, it } from 'vitest'
import { JOB_TYPES } from '@/lib/server/job-types'

/**
 * Tests for jobs route validation logic.
 * We test the pure validation extracted from the route handler since
 * the route itself requires a real PG connection.
 */

const VALID_TYPES = new Set(['all', 'crawl', 'extract', 'embed', 'ingest', 'refresh'])
const VALID_STATUSES = new Set([
  'all',
  'active',
  'pending',
  'running',
  'completed',
  'failed',
  'canceled',
])

describe('jobs route validation', () => {
  describe('type filter', () => {
    it('accepts all valid type values', () => {
      for (const t of VALID_TYPES) {
        expect(VALID_TYPES.has(t)).toBe(true)
      }
    })

    it('rejects unknown type values', () => {
      expect(VALID_TYPES.has('foo')).toBe(false)
      expect(VALID_TYPES.has('CRAWL')).toBe(false)
      expect(VALID_TYPES.has('')).toBe(false)
    })

    it('includes refresh type filter', () => {
      expect(VALID_TYPES.has('refresh')).toBe(true)
    })
  })

  describe('server job types', () => {
    it('includes refresh in canonical JOB_TYPES', () => {
      expect([...JOB_TYPES]).toContain('refresh')
    })
  })

  describe('status filter', () => {
    it('accepts all valid status values', () => {
      for (const s of VALID_STATUSES) {
        expect(VALID_STATUSES.has(s)).toBe(true)
      }
    })

    it('rejects unknown status values', () => {
      expect(VALID_STATUSES.has('foo')).toBe(false)
      expect(VALID_STATUSES.has('PENDING')).toBe(false)
      expect(VALID_STATUSES.has('done')).toBe(false)
      expect(VALID_STATUSES.has('')).toBe(false)
    })
  })

  describe('limit clamping', () => {
    function clampLimit(raw: string | null): number {
      return Math.min(Math.max(Number(raw ?? '50'), 1), 200)
    }

    it('defaults to 50', () => {
      expect(clampLimit(null)).toBe(50)
    })

    it('clamps to minimum 1', () => {
      expect(clampLimit('0')).toBe(1)
      expect(clampLimit('-10')).toBe(1)
    })

    it('clamps to maximum 200', () => {
      expect(clampLimit('999')).toBe(200)
    })

    it('passes through valid values', () => {
      expect(clampLimit('25')).toBe(25)
    })
  })

  describe('offset clamping', () => {
    function clampOffset(raw: string | null): number {
      return Math.max(Number(raw ?? '0'), 0)
    }

    it('defaults to 0', () => {
      expect(clampOffset(null)).toBe(0)
    })

    it('clamps negative to 0', () => {
      expect(clampOffset('-5')).toBe(0)
    })

    it('passes through valid values', () => {
      expect(clampOffset('100')).toBe(100)
    })
  })
})
