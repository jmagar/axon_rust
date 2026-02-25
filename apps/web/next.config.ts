import type { NextConfig } from 'next'

const axonPort = process.env.NEXT_PUBLIC_AXON_PORT || '3939'

const nextConfig: NextConfig = {
  output: 'standalone',
  turbopack: {
    root: __dirname,
  },
  async rewrites() {
    return [
      {
        source: '/ws',
        destination: `http://localhost:${axonPort}/ws`,
      },
      {
        source: '/download/:path*',
        destination: `http://localhost:${axonPort}/download/:path*`,
      },
      {
        source: '/output/:path*',
        destination: `http://localhost:${axonPort}/output/:path*`,
      },
    ]
  },
}

export default nextConfig
