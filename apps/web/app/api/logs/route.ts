import { PassThrough, type Readable } from 'node:stream'
import Dockerode from 'dockerode'
import type { NextRequest } from 'next/server'

// biome-ignore lint/suspicious/noControlCharactersInRegex: intentional ANSI escape sequence stripping
const ANSI_RE = /\x1b\[[0-9;]*[mGKHFJABCDfnsuhl]/g
function stripAnsi(s: string): string {
  return s.replace(ANSI_RE, '')
}

export const dynamic = 'force-dynamic'

export const SERVICES = [
  'axon-workers',
  'axon-web',
  'axon-postgres',
  'axon-redis',
  'axon-rabbitmq',
  'axon-qdrant',
  'axon-chrome',
] as const

const ALLOWED_SERVICES = new Set<string>(SERVICES)

/**
 * SECURITY: Docker socket grants full Docker API access. This route is scoped
 * to read-only container log streaming (getContainer().logs()) against the
 * ALLOWED_SERVICES allowlist. No exec, stop, remove, or image operations.
 * Auth is enforced by middleware.ts (AXON_WEB_API_TOKEN).
 */
const docker = new Dockerode({ socketPath: '/var/run/docker.sock' })

type SendLine = (line: string, service?: string) => void

function attachContainerStream(
  svc: string,
  tail: number,
  sendLine: SendLine,
  onDone: () => void,
  logStreams: Readable[],
): void {
  docker
    .getContainer(svc)
    .logs({ follow: true, stdout: true, stderr: true, tail })
    .then((raw) => {
      const logStream = raw as Readable
      logStreams.push(logStream)

      const pt = new PassThrough()
      docker.modem.demuxStream(logStream, pt, pt)

      pt.on('data', (chunk: Buffer) => {
        for (const line of chunk.toString().split('\n')) {
          const clean = stripAnsi(line)
          if (clean.trim()) sendLine(clean, svc)
        }
      })
      pt.on('end', onDone)
      pt.on('error', (err: Error) => {
        sendLine(`[stream error] ${err.message}`, svc)
        onDone()
      })
    })
    .catch((err: unknown) => {
      sendLine(`[stream error] ${err instanceof Error ? err.message : String(err)}`, svc)
      onDone()
    })
}

export async function GET(req: NextRequest) {
  const service = req.nextUrl.searchParams.get('service') ?? 'axon-workers'
  const tail = Math.min(Number(req.nextUrl.searchParams.get('tail') ?? '200'), 1000)
  const isAll = service === 'all'

  if (!isAll && !ALLOWED_SERVICES.has(service)) {
    return new Response('Invalid service', { status: 400 })
  }

  if (!Number.isFinite(tail) || tail < 1) {
    return new Response('Invalid tail value', { status: 400 })
  }

  const targets: string[] = isAll ? [...SERVICES] : [service]
  const encoder = new TextEncoder()
  const logStreams: Readable[] = []

  const stream = new ReadableStream({
    start(controller) {
      function sendLine(line: string, svc?: string) {
        const payload = JSON.stringify({
          line,
          ts: Date.now(),
          ...(svc && isAll ? { service: svc } : {}),
        })
        controller.enqueue(encoder.encode(`data: ${payload}\n\n`))
      }

      function close() {
        try {
          controller.close()
        } catch {
          // already closed
        }
      }

      let active = targets.length

      function onDone() {
        if (--active <= 0) close()
      }

      for (const svc of targets) {
        attachContainerStream(svc, tail, sendLine, onDone, logStreams)
      }

      req.signal.addEventListener('abort', () => {
        for (const s of logStreams) s.destroy()
        close()
      })
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
