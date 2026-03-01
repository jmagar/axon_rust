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

const server = createServer((_req, res) => {
  res.writeHead(200).end('axon shell-server ok')
})

const wss = new WebSocketServer({ server })

wss.on('connection', (ws) => {
  const term = pty.spawn(SHELL, [], {
    name: 'xterm-256color',
    cols: 80,
    rows: 24,
    cwd: process.env.HOME ?? '/home/node',
    env: {
      ...process.env,
      TERM: 'xterm-256color',
      COLORTERM: 'truecolor',
    },
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
