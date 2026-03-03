/**
 * SSRF validation for user-supplied URLs before passing them to subprocesses.
 *
 * Blocks private/reserved IP ranges and non-HTTP(S) schemes.
 *
 * Known limitation: DNS rebinding attacks cannot be prevented at URL-parse time.
 * A hostname that resolves to a public IP during validation may later resolve to
 * a private IP when the subprocess fetches it. Mitigating this requires runtime
 * DNS pinning or connect-time IP checks, which are outside the scope of this module.
 */

const BLOCKED_HOSTNAMES = new Set([
  'localhost',
  'localhost.localdomain',
  '0.0.0.0',
  '[::1]',
  '[::0]',
  '[0:0:0:0:0:0:0:0]',
  '[0:0:0:0:0:0:0:1]',
])

/** IPv4 ranges that are private/reserved (RFC 1918, link-local, loopback, etc.) */
function isPrivateIpv4(ip: string): boolean {
  const parts = ip.split('.').map(Number)
  if (parts.length !== 4 || parts.some((p) => Number.isNaN(p) || p < 0 || p > 255)) return false

  const [a, b] = parts
  // 10.0.0.0/8
  if (a === 10) return true
  // 172.16.0.0/12
  if (a === 172 && b !== undefined && b >= 16 && b <= 31) return true
  // 192.168.0.0/16
  if (a === 192 && b === 168) return true
  // 127.0.0.0/8 (loopback)
  if (a === 127) return true
  // 169.254.0.0/16 (link-local)
  if (a === 169 && b === 254) return true
  // 0.0.0.0/8
  if (a === 0) return true

  return false
}

/**
 * Parse an IPv6 address string into 8 16-bit groups.
 * Handles :: expansion. Returns null if not a valid IPv6 address.
 */
function parseIpv6(addr: string): number[] | null {
  // Strip brackets if present
  const raw = addr.startsWith('[') ? addr.slice(1, -1) : addr

  // Split on ::
  const halves = raw.split('::')
  if (halves.length > 2) return null

  const left = halves[0] ? halves[0].split(':') : []
  const right = halves[1] !== undefined && halves[1] !== '' ? halves[1].split(':') : []

  const specified = left.length + right.length
  if (halves.length === 1 && specified !== 8) return null
  if (halves.length === 2 && specified > 7) return null

  const groups: number[] = []
  for (const g of left) {
    const n = Number.parseInt(g, 16)
    if (Number.isNaN(n) || n < 0 || n > 0xffff) return null
    groups.push(n)
  }

  // Fill :: gap with zeros
  if (halves.length === 2) {
    const gap = 8 - specified
    for (let i = 0; i < gap; i++) groups.push(0)
  }

  for (const g of right) {
    const n = Number.parseInt(g, 16)
    if (Number.isNaN(n) || n < 0 || n > 0xffff) return null
    groups.push(n)
  }

  if (groups.length !== 8) return null
  return groups
}

/**
 * Check if an IPv6 address is in a blocked range:
 * - ::ffff:0:0/96 (IPv6-mapped IPv4) — delegates to isPrivateIpv4
 * - fc00::/7 (ULA — unique local addresses)
 * - fe80::/10 (link-local)
 * - ff00::/8 (multicast)
 * - ::1 (loopback — caught by BLOCKED_HOSTNAMES but checked here for completeness)
 * - :: (unspecified)
 */
function isBlockedIpv6(hostname: string): { blocked: boolean; reason?: string } {
  const groups = parseIpv6(hostname)
  if (!groups) return { blocked: false }

  // Loopback ::1
  if (groups.every((g, i) => (i < 7 ? g === 0 : g === 1))) {
    return { blocked: true, reason: 'Blocked IPv6 loopback: ::1' }
  }

  // Unspecified ::
  if (groups.every((g) => g === 0)) {
    return { blocked: true, reason: 'Blocked IPv6 unspecified: ::' }
  }

  // IPv6-mapped IPv4 — ::ffff:x.x.x.x (groups 0-4 are 0, group 5 is 0xffff)
  if (
    groups[0] === 0 &&
    groups[1] === 0 &&
    groups[2] === 0 &&
    groups[3] === 0 &&
    groups[4] === 0 &&
    groups[5] === 0xffff
  ) {
    // Extract the IPv4 address from the last two groups
    const a = (groups[6] >> 8) & 0xff
    const b = groups[6] & 0xff
    const c = (groups[7] >> 8) & 0xff
    const d = groups[7] & 0xff
    const ipv4 = `${a}.${b}.${c}.${d}`
    if (isPrivateIpv4(ipv4)) {
      return { blocked: true, reason: `Blocked IPv6-mapped private IPv4: ${ipv4}` }
    }
    return { blocked: false }
  }

  // fc00::/7 — ULA (first byte fc or fd, i.e. first group >> 8 is 0xfc or 0xfd)
  const firstByte = groups[0] >> 8
  if (firstByte === 0xfc || firstByte === 0xfd) {
    return { blocked: true, reason: `Blocked IPv6 ULA: ${hostname}` }
  }

  // fe80::/10 — link-local (first 10 bits = 0x3fa = 1111111010)
  if ((groups[0] & 0xffc0) === 0xfe80) {
    return { blocked: true, reason: `Blocked IPv6 link-local: ${hostname}` }
  }

  // ff00::/8 — multicast
  if (firstByte === 0xff) {
    return { blocked: true, reason: `Blocked IPv6 multicast: ${hostname}` }
  }

  return { blocked: false }
}

export interface UrlValidationResult {
  valid: boolean
  reason?: string
}

/**
 * Validate a URL for safety before passing to a subprocess (e.g. axon scrape).
 *
 * Rejects:
 * - Non-HTTP(S) schemes (file://, ftp://, data:, javascript:, etc.)
 * - Private/reserved IPv4 addresses
 * - Known loopback hostnames
 * - URLs without a valid hostname
 */
export function validateUrlForSsrf(raw: string): UrlValidationResult {
  let parsed: URL
  try {
    parsed = new URL(raw)
  } catch {
    return { valid: false, reason: 'Malformed URL' }
  }

  // Scheme check
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    return { valid: false, reason: `Blocked scheme: ${parsed.protocol}` }
  }

  const hostname = parsed.hostname.toLowerCase()

  if (!hostname) {
    return { valid: false, reason: 'Missing hostname' }
  }

  // Known blocked hostnames
  if (BLOCKED_HOSTNAMES.has(hostname)) {
    return { valid: false, reason: `Blocked hostname: ${hostname}` }
  }

  // IPv4 private range check
  if (isPrivateIpv4(hostname)) {
    return { valid: false, reason: `Blocked private IP: ${hostname}` }
  }

  // IPv6 range checks (mapped IPv4, ULA, link-local, multicast)
  const ipv6Check = isBlockedIpv6(hostname)
  if (ipv6Check.blocked) {
    return { valid: false, reason: ipv6Check.reason }
  }

  return { valid: true }
}

/**
 * Validate an array of URLs. Returns the first failure or `{ valid: true }`.
 */
export function validateUrlsForSsrf(urls: string[]): UrlValidationResult & { url?: string } {
  for (const url of urls) {
    const result = validateUrlForSsrf(url)
    if (!result.valid) {
      return { ...result, url }
    }
  }
  return { valid: true }
}
