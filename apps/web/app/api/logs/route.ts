import type { Readable } from 'node:stream'
import Dockerode from 'dockerode'
import type { NextRequest } from 'next/server'

// biome-ignore lint/suspicious/noControlCharactersInRegex: intentional ANSI escape sequence stripping
const ANSI_RE = /\x1b\[[0-9;]*[mGKHFJABCDfnsuhl]/g
function stripAnsi(s: string): string {
  return s.replace(ANSI_RE, '')
}

export const dynamic = 'force-dynamic'

const ALLOWED_SERVICES = new Set([
  'axon-postgres',
  'axon-redis',
  'axon-rabbitmq',
  'axon-qdrant',
  'axon-chrome',
  'axon-workers',
  'axon-web',
])

const docker = new Dockerode({ socketPath: '/var/run/docker.sock' })

export async function GET(req: NextRequest) {
  const service = req.nextUrl.searchParams.get('service') ?? 'axon-workers'
  const tail = Math.min(Number(req.nextUrl.searchParams.get('tail') ?? '200'), 1000)

  if (!ALLOWED_SERVICES.has(service)) {
    return new Response('Invalid service', { status: 400 })
  }

  if (!Number.isFinite(tail) || tail < 1) {
    return new Response('Invalid tail value', { status: 400 })
  }

  const encoder = new TextEncoder()

  const stream = new ReadableStream({
    async start(controller) {
      function sendLine(line: string) {
        const payload = JSON.stringify({ line, ts: Date.now() })
        controller.enqueue(encoder.encode(`data: ${payload}\n\n`))
      }

      function close() {
        try {
          controller.close()
        } catch {
          // already closed
        }
      }

      try {
        const container = docker.getContainer(service)
        const logStream = (await container.logs({
          follow: true,
          stdout: true,
          stderr: true,
          tail,
        })) as Readable

        // Docker multiplexes stdout/stderr in an 8-byte frame header when not in TTY mode.
        // demuxStream splits them into separate writable streams.
        const passThrough = new (await import('node:stream')).PassThrough()

        docker.modem.demuxStream(logStream, passThrough, passThrough)

        passThrough.on('data', (chunk: Buffer) => {
          for (const line of chunk.toString().split('\n')) {
            const clean = stripAnsi(line)
            if (clean.trim()) sendLine(clean)
          }
        })

        passThrough.on('end', close)
        passThrough.on('error', (err: Error) => {
          sendLine(`[stream error] ${err.message}`)
          close()
        })

        req.signal.addEventListener('abort', () => {
          logStream.destroy()
          close()
        })
      } catch (err) {
        sendLine(`[stream error] ${err instanceof Error ? err.message : String(err)}`)
        close()
      }
    },
  })

  return new Response(stream, {
    headers: {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      Connection: 'keep-alive',
    },
  })
}
