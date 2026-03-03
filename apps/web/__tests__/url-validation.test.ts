import { describe, expect, it } from 'vitest'
import { validateUrlForSsrf, validateUrlsForSsrf } from '@/lib/server/url-validation'

describe('validateUrlForSsrf', () => {
  it('allows valid HTTPS URLs', () => {
    expect(validateUrlForSsrf('https://example.com')).toEqual({ valid: true })
    expect(validateUrlForSsrf('https://docs.rust-lang.org/book/')).toEqual({ valid: true })
  })

  it('allows valid HTTP URLs', () => {
    expect(validateUrlForSsrf('http://example.com')).toEqual({ valid: true })
  })

  it('blocks non-HTTP schemes', () => {
    const file = validateUrlForSsrf('file:///etc/passwd')
    expect(file.valid).toBe(false)
    expect(file.reason).toContain('Blocked scheme')

    const ftp = validateUrlForSsrf('ftp://ftp.example.com')
    expect(ftp.valid).toBe(false)

    const data = validateUrlForSsrf('data:text/html,<h1>hi</h1>')
    expect(data.valid).toBe(false)
  })

  it('blocks localhost', () => {
    const r = validateUrlForSsrf('http://localhost:8080/secret')
    expect(r.valid).toBe(false)
    expect(r.reason).toContain('localhost')
  })

  it('blocks 127.0.0.1', () => {
    const r = validateUrlForSsrf('http://127.0.0.1:3000/admin')
    expect(r.valid).toBe(false)
    expect(r.reason).toContain('127.0.0.1')
  })

  it('blocks 0.0.0.0', () => {
    const r = validateUrlForSsrf('http://0.0.0.0/')
    expect(r.valid).toBe(false)
  })

  it('blocks private 10.x.x.x range', () => {
    const r = validateUrlForSsrf('http://10.0.0.1:9200/elasticsearch')
    expect(r.valid).toBe(false)
    expect(r.reason).toContain('private IP')
  })

  it('blocks private 172.16-31.x.x range', () => {
    expect(validateUrlForSsrf('http://172.16.0.1/').valid).toBe(false)
    expect(validateUrlForSsrf('http://172.31.255.255/').valid).toBe(false)
    // 172.32.x.x is NOT private
    expect(validateUrlForSsrf('http://172.32.0.1/').valid).toBe(true)
  })

  it('blocks private 192.168.x.x range', () => {
    const r = validateUrlForSsrf('http://192.168.1.1/')
    expect(r.valid).toBe(false)
  })

  it('blocks link-local 169.254.x.x', () => {
    const r = validateUrlForSsrf('http://169.254.169.254/latest/meta-data/')
    expect(r.valid).toBe(false)
  })

  it('blocks IPv6 loopback', () => {
    const r = validateUrlForSsrf('http://[::1]:8080/')
    expect(r.valid).toBe(false)
  })

  it('blocks IPv6-mapped IPv4 loopback (::ffff:127.0.0.1)', () => {
    // Node URL parser normalises http://[::ffff:7f00:0001]/ to hostname ::ffff:7f00:1
    const r = validateUrlForSsrf('http://[::ffff:7f00:1]/')
    expect(r.valid).toBe(false)
    expect(r.reason).toContain('IPv6-mapped')
  })

  it('blocks IPv6-mapped private 10.x (::ffff:a00:1)', () => {
    const r = validateUrlForSsrf('http://[::ffff:a00:1]/')
    expect(r.valid).toBe(false)
    expect(r.reason).toContain('10.0.0.1')
  })

  it('blocks IPv6-mapped private 192.168.x (::ffff:c0a8:101)', () => {
    const r = validateUrlForSsrf('http://[::ffff:c0a8:101]/')
    expect(r.valid).toBe(false)
    expect(r.reason).toContain('192.168.1.1')
  })

  it('allows IPv6-mapped public IP', () => {
    // ::ffff:8.8.8.8 = ::ffff:808:808
    const r = validateUrlForSsrf('http://[::ffff:808:808]/')
    expect(r.valid).toBe(true)
  })

  it('blocks IPv6 ULA fc00::/7', () => {
    expect(validateUrlForSsrf('http://[fc00::1]/').valid).toBe(false)
    expect(validateUrlForSsrf('http://[fd00::1]/').valid).toBe(false)
    expect(validateUrlForSsrf('http://[fdab:1234::1]/').valid).toBe(false)
  })

  it('blocks IPv6 link-local fe80::/10', () => {
    const r = validateUrlForSsrf('http://[fe80::1]/')
    expect(r.valid).toBe(false)
    expect(r.reason).toContain('link-local')
  })

  it('blocks IPv6 multicast ff00::/8', () => {
    const r = validateUrlForSsrf('http://[ff02::1]/')
    expect(r.valid).toBe(false)
    expect(r.reason).toContain('multicast')
  })

  it('allows valid public IPv6', () => {
    // 2001:db8:: is documentation range but not blocked by SSRF rules
    expect(validateUrlForSsrf('http://[2607:f8b0:4004:800::200e]/').valid).toBe(true)
  })

  it('rejects malformed URLs', () => {
    const r = validateUrlForSsrf('not-a-url')
    expect(r.valid).toBe(false)
    expect(r.reason).toContain('Malformed')
  })
})

describe('validateUrlsForSsrf', () => {
  it('returns valid for all-good URLs', () => {
    const r = validateUrlsForSsrf(['https://a.com', 'https://b.com'])
    expect(r.valid).toBe(true)
  })

  it('returns first failure with offending URL', () => {
    const r = validateUrlsForSsrf([
      'https://good.com',
      'http://127.0.0.1/',
      'https://also-good.com',
    ])
    expect(r.valid).toBe(false)
    expect(r.url).toBe('http://127.0.0.1/')
  })

  it('returns valid for empty array', () => {
    expect(validateUrlsForSsrf([]).valid).toBe(true)
  })
})
