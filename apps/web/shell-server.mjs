/**
 * Standalone WebSocket PTY shell server.
 * Runs inside the axon-web container so the terminal lands in the right environment.
 * Proxied from Next.js via the /ws/shell rewrite → http://localhost:49011
 *
 * Protocol (mirrors use-shell-session.ts expectations):
 *   client → server: { type: "input",  data: string }
 *   client → server: { type: "resize", cols: number, rows: number }
 *   server → client: { type: "output", data: string }
 */

import { createServer } from 'node:http'
import pty from 'node-pty'
import { WebSocketServer } from 'ws'

const PORT = Number(process.env.SHELL_SERVER_PORT ?? 49011)
const SHELL = process.env.SHELL ?? '/bin/bash'
const TOKEN = process.env.AXON_SHELL_WS_TOKEN ?? process.env.AXON_WEB_API_TOKEN ?? ''
const ALLOWED_ORIGINS = (
  process.env.AXON_SHELL_ALLOWED_ORIGINS ??
  process.env.AXON_WEB_ALLOWED_ORIGINS ??
  ''
)
  .split(',')
  .map((value) => value.trim())
  .filter(Boolean)
const ALLOW_INSECURE_LOCAL_DEV = process.env.AXON_WEB_ALLOW_INSECURE_DEV === 'true'
const SAFE_ENV_KEYS = [
  'HOME',
  'PATH',
  'SHELL',
  'LANG',
  'LC_ALL',
  'LC_CTYPE',
  'TZ',
  'TMPDIR',
  'PWD',
  'USER',
  'USERNAME',
]

function isLoopbackHost(host) {
  return (
    host === 'localhost' ||
    host === '127.0.0.1' ||
    host === '::1' ||
    host === '[::1]' ||
    host === '0.0.0.0'
  )
}

function parseOrigin(originHeader) {
  if (!originHeader) return null
  try {
    return new URL(originHeader)
  } catch {
    return null
  }
}

function isAllowedOrigin(req) {
  const parsedOrigin = parseOrigin(req.headers.origin)
  if (!parsedOrigin) return true

  const normalized = parsedOrigin.origin.toLowerCase()
  if (ALLOWED_ORIGINS.length > 0) {
    return ALLOWED_ORIGINS.some((allowed) => allowed.toLowerCase() === normalized)
  }

  if (ALLOW_INSECURE_LOCAL_DEV) {
    return isLoopbackHost(parsedOrigin.hostname)
  }

  const requestHost = String(req.headers.host ?? '')
    .split(':')[0]
    .toLowerCase()
  return parsedOrigin.hostname.toLowerCase() === requestHost
}

function getAuthToken(req) {
  const authHeader = req.headers.authorization
  if (typeof authHeader === 'string' && authHeader.startsWith('Bearer ')) {
    return authHeader.slice('Bearer '.length).trim()
  }

  const apiKey = req.headers['x-api-key']
  if (typeof apiKey === 'string' && apiKey.trim()) {
    return apiKey.trim()
  }

  try {
    const url = new URL(req.url ?? '/', `http://${req.headers.host ?? 'localhost'}`)
    return url.searchParams.get('token')?.trim() ?? ''
  } catch {
    return ''
  }
}

function isAuthorized(req) {
  if (TOKEN) return getAuthToken(req) === TOKEN
  if (!ALLOW_INSECURE_LOCAL_DEV) return false

  const host = (req.headers.host ?? '').split(':')[0] ?? ''
  return isLoopbackHost(host)
}

function buildShellEnv() {
  const env = {}
  for (const key of SAFE_ENV_KEYS) {
    const value = process.env[key]
    if (typeof value === 'string' && value.length > 0) {
      env[key] = value
    }
  }
  env.TERM = 'xterm-256color'
  env.COLORTERM = 'truecolor'
  return env
}

const server = createServer((_req, res) => {
  res.writeHead(200).end('axon shell-server ok')
})

const wss = new WebSocketServer({ noServer: true })

server.on('upgrade', (req, socket, head) => {
  if (!isAllowedOrigin(req)) {
    socket.write('HTTP/1.1 403 Forbidden\r\n\r\n')
    socket.destroy()
    return
  }
  if (!isAuthorized(req)) {
    socket.write('HTTP/1.1 401 Unauthorized\r\n\r\n')
    socket.destroy()
    return
  }
  wss.handleUpgrade(req, socket, head, (ws) => {
    wss.emit('connection', ws, req)
  })
})

wss.on('connection', (ws) => {
  const term = pty.spawn(SHELL, [], {
    name: 'xterm-256color',
    cols: 80,
    rows: 24,
    cwd: process.env.HOME ?? '/home/node',
    env: buildShellEnv(),
  })

  term.onData((data) => {
    if (ws.readyState === ws.OPEN) {
      ws.send(JSON.stringify({ type: 'output', data }))
    }
  })

  term.onExit(() => {
    if (ws.readyState === ws.OPEN) ws.close()
  })

  ws.on('message', (raw) => {
    try {
      const msg = JSON.parse(String(raw))
      if (msg.type === 'input' && typeof msg.data === 'string') {
        term.write(msg.data)
      } else if (msg.type === 'resize' && msg.cols && msg.rows) {
        term.resize(Number(msg.cols), Number(msg.rows))
      }
    } catch {
      /* ignore malformed messages */
    }
  })

  ws.on('close', () => term.kill())
  ws.on('error', () => term.kill())
})

server.listen(PORT, '127.0.0.1', () => {
  console.log(`[shell-server] listening on 127.0.0.1:${PORT}`)
})
