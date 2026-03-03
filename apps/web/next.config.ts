import type { NextConfig } from 'next'

const axonBackendUrl =
  process.env.AXON_BACKEND_URL || `http://localhost:${process.env.NEXT_PUBLIC_AXON_PORT || '3939'}`
const isDev = process.env.NODE_ENV !== 'production'

const securityHeaders = [
  { key: 'X-Frame-Options', value: 'DENY' },
  { key: 'X-Content-Type-Options', value: 'nosniff' },
  { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
  { key: 'Permissions-Policy', value: 'camera=(), microphone=(), geolocation=()' },
  {
    key: 'Content-Security-Policy',
    value: [
      "default-src 'self'",
      "base-uri 'self'",
      "form-action 'self'",
      "frame-ancestors 'none'",
      "object-src 'none'",
      `script-src 'self' 'unsafe-inline'${isDev ? " 'unsafe-eval'" : ''}`,
      "style-src 'self' 'unsafe-inline'",
      "img-src 'self' data: blob: https:",
      "font-src 'self' data:",
      "connect-src 'self' https: http: ws: wss:",
    ].join('; '),
  },
  ...(isDev
    ? []
    : [{ key: 'Strict-Transport-Security', value: 'max-age=31536000; includeSubDomains' }]),
]

const nextConfig: NextConfig = {
  output: 'standalone',
  transpilePackages: [
    '@platejs/ai',
    '@platejs/basic-nodes',
    '@platejs/code-block',
    '@platejs/link',
    '@platejs/list',
    '@platejs/markdown',
    '@platejs/media',
    '@platejs/table',
    'platejs',
  ],
  turbopack: {
    root: __dirname,
  },
  async headers() {
    return [
      {
        source: '/:path*',
        headers: securityHeaders,
      },
      {
        source: '/sw.js',
        headers: [
          { key: 'Cache-Control', value: 'no-cache, no-store, must-revalidate' },
          { key: 'Service-Worker-Allowed', value: '/' },
        ],
      },
      {
        source: '/api/cortex/:path*',
        headers: [
          { key: 'Cache-Control', value: 'public, s-maxage=30, stale-while-revalidate=60' },
        ],
      },
    ]
  },
  async rewrites() {
    return [
      {
        source: '/ws',
        destination: `${axonBackendUrl}/ws`,
      },
      {
        source: '/ws/shell',
        destination: `http://127.0.0.1:${process.env.SHELL_SERVER_PORT ?? 49011}`,
      },
      {
        source: '/download/:path*',
        destination: `${axonBackendUrl}/download/:path*`,
      },
      {
        source: '/output/:path*',
        destination: `${axonBackendUrl}/output/:path*`,
      },
    ]
  },
}

export default nextConfig
