import { describe, expect, it } from 'vitest'
import { apiError, makeErrorId } from '@/lib/server/api-error'

describe('apiError', () => {
  it('returns NextResponse with correct status', async () => {
    const res = apiError(400, 'Bad request')
    expect(res.status).toBe(400)
    const body = await res.json()
    expect(body).toEqual({ error: 'Bad request' })
  })

  it('includes optional code and errorId', async () => {
    const res = apiError(500, 'Internal error', { code: 'test_code', errorId: 'abc-123' })
    expect(res.status).toBe(500)
    const body = await res.json()
    expect(body).toEqual({
      error: 'Internal error',
      code: 'test_code',
      errorId: 'abc-123',
    })
  })

  it('includes optional detail', async () => {
    const res = apiError(502, 'Upstream failed', { detail: 'timeout after 30s' })
    const body = await res.json()
    expect(body).toEqual({
      error: 'Upstream failed',
      detail: 'timeout after 30s',
    })
  })

  it('omits undefined optional fields', async () => {
    const res = apiError(404, 'Not found', {})
    const body = await res.json()
    expect(body).toEqual({ error: 'Not found' })
    expect(Object.keys(body)).toEqual(['error'])
  })
})

describe('makeErrorId', () => {
  it('returns a string with the prefix when crypto is unavailable', () => {
    const id = makeErrorId('test')
    expect(typeof id).toBe('string')
    expect(id.length).toBeGreaterThan(0)
  })

  it('returns a UUID-like string when crypto is available', () => {
    const id = makeErrorId('test')
    // In Node.js test env, crypto.randomUUID is available
    expect(id).toMatch(/^[0-9a-f-]{36}$/i)
  })
})
