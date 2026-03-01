import type { NextConfig } from 'next'

const axonBackendUrl =
  process.env.AXON_BACKEND_URL || `http://localhost:${process.env.NEXT_PUBLIC_AXON_PORT || '3939'}`

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
        source: '/sw.js',
        headers: [
          { key: 'Cache-Control', value: 'no-cache, no-store, must-revalidate' },
          { key: 'Service-Worker-Allowed', value: '/' },
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
