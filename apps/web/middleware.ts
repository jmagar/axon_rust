import type { NextRequest } from 'next/server'
import { NextResponse } from 'next/server'

const API_TOKEN = process.env.AXON_WEB_API_TOKEN?.trim() || null
const ALLOWED_ORIGINS = (process.env.AXON_WEB_ALLOWED_ORIGINS ?? '')
  .split(',')
  .map((value) => value.trim().toLowerCase())
  .filter(Boolean)
const ALLOW_INSECURE_LOCAL_DEV = process.env.AXON_WEB_ALLOW_INSECURE_DEV === 'true'
const IS_DEV = process.env.NODE_ENV !== 'production'

const SECURITY_HEADERS: ReadonlyArray<readonly [string, string]> = [
  ['X-Frame-Options', 'DENY'],
  ['X-Content-Type-Options', 'nosniff'],
  ['Referrer-Policy', 'strict-origin-when-cross-origin'],
  ['Permissions-Policy', 'camera=(), microphone=(), geolocation=()'],
  [
    'Content-Security-Policy',
    [
      "default-src 'self'",
      "base-uri 'self'",
      "frame-ancestors 'none'",
      "object-src 'none'",
      "img-src 'self' data: blob:",
      "font-src 'self' data:",
      IS_DEV
        ? "script-src 'self' 'unsafe-inline' 'unsafe-eval'"
        : "script-src 'self' 'unsafe-inline'",
      "style-src 'self' 'unsafe-inline'",
      "connect-src 'self' ws: wss: http: https:",
    ].join('; '),
  ],
]

function withSecurityHeaders(response: NextResponse): NextResponse {
  for (const [key, value] of SECURITY_HEADERS) {
    response.headers.set(key, value)
  }
  if (!IS_DEV) {
    response.headers.set('Strict-Transport-Security', 'max-age=31536000; includeSubDomains')
  }
  return response
}

function isLoopbackHost(host: string): boolean {
  return host === 'localhost' || host === '127.0.0.1' || host === '::1' || host === '[::1]'
}

function getRequestHost(req: NextRequest): string {
  return req.headers.get('x-forwarded-host') ?? req.nextUrl.hostname
}

function isLocalhostRequest(req: NextRequest): boolean {
  const host = getRequestHost(req).split(':')[0]?.toLowerCase() ?? ''
  return isLoopbackHost(host)
}

function isAllowedOrigin(req: NextRequest): boolean {
  const origin = req.headers.get('origin')
  if (!origin) return true

  let parsed: URL
  try {
    parsed = new URL(origin)
  } catch {
    return false
  }

  const normalizedOrigin = parsed.origin.toLowerCase()
  if (ALLOWED_ORIGINS.length > 0) {
    return ALLOWED_ORIGINS.includes(normalizedOrigin)
  }

  if (ALLOW_INSECURE_LOCAL_DEV && isLoopbackHost(parsed.hostname.toLowerCase())) {
    return true
  }

  const requestOrigin = `${req.nextUrl.protocol}//${getRequestHost(req)}`.toLowerCase()
  return normalizedOrigin === requestOrigin
}

function extractToken(req: NextRequest): string {
  const authHeader = req.headers.get('authorization')
  if (authHeader?.startsWith('Bearer ')) {
    return authHeader.slice('Bearer '.length).trim()
  }

  const key = req.headers.get('x-api-key')
  return key?.trim() ?? ''
}

function isAuthorized(req: NextRequest): boolean {
  if (API_TOKEN !== null) {
    return extractToken(req) === API_TOKEN
  }

  return ALLOW_INSECURE_LOCAL_DEV && isLocalhostRequest(req)
}

export function middleware(req: NextRequest) {
  if (!isAllowedOrigin(req)) {
    return withSecurityHeaders(NextResponse.json({ error: 'Forbidden origin' }, { status: 403 }))
  }

  if (!isAuthorized(req)) {
    if (!API_TOKEN && !ALLOW_INSECURE_LOCAL_DEV) {
      return withSecurityHeaders(
        NextResponse.json(
          {
            error:
              'API authentication is not configured. Set AXON_WEB_API_TOKEN or enable AXON_WEB_ALLOW_INSECURE_DEV=true for localhost development.',
          },
          { status: 503 },
        ),
      )
    }
    return withSecurityHeaders(NextResponse.json({ error: 'Unauthorized' }, { status: 401 }))
  }

  return withSecurityHeaders(NextResponse.next())
}

export const config = {
  matcher: ['/api/:path*'],
}
