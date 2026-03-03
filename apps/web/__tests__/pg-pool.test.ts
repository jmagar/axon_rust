import { beforeEach, describe, expect, it, vi } from 'vitest'

/**
 * Tests for PG pool singleton behavior.
 * We mock the pg module to verify the pool is created only once.
 */

vi.mock('pg', () => {
  class MockPool {
    query = vi.fn()
    end = vi.fn()
  }
  return { Pool: MockPool }
})

describe('getJobsPgPool', () => {
  beforeEach(() => {
    // Clear the singleton between tests
    const g = globalThis as { __axonJobsPgPool?: unknown }
    delete g.__axonJobsPgPool
    vi.resetModules()
  })

  it('returns a Pool instance', async () => {
    const { getJobsPgPool } = await import('@/lib/server/pg-pool')
    const pool = getJobsPgPool()
    expect(pool).toBeDefined()
    expect(pool).toHaveProperty('query')
  })

  it('returns the same instance on repeated calls (singleton)', async () => {
    const { getJobsPgPool } = await import('@/lib/server/pg-pool')
    const a = getJobsPgPool()
    const b = getJobsPgPool()
    expect(a).toBe(b)
  })

  it('caches the pool on globalThis', async () => {
    const { getJobsPgPool } = await import('@/lib/server/pg-pool')
    getJobsPgPool()
    const g = globalThis as { __axonJobsPgPool?: unknown }
    expect(g.__axonJobsPgPool).toBeDefined()
  })
})
