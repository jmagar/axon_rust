/**
 * Execute an axon command via the axon-workers WebSocket bridge.
 *
 * Rather than spawning the axon binary from axon-web (where the binary may
 * not exist), this connects to the WS execution bridge that runs inside the
 * axon-workers container, which always has the binary available.
 */

const WORKERS_WS_URL = process.env.AXON_WORKERS_WS_URL ?? 'ws://axon-workers:49000/ws'
const WORKERS_WS_TOKEN = process.env.AXON_WEB_API_TOKEN?.trim() ?? ''

function buildWorkersWsUrl(): string {
  if (!WORKERS_WS_TOKEN) return WORKERS_WS_URL
  try {
    const url = new URL(WORKERS_WS_URL)
    if (!url.searchParams.has('token')) {
      url.searchParams.set('token', WORKERS_WS_TOKEN)
    }
    return url.toString()
  } catch {
    // Fallback for malformed env URL values; preserve current behavior.
    return WORKERS_WS_URL
  }
}

interface WsMessageEvent {
  data: unknown
}

interface WsCloseEvent {
  code: number
}

interface WsLike {
  addEventListener(type: 'open', listener: () => void): void
  addEventListener(type: 'message', listener: (event: WsMessageEvent) => void): void
  addEventListener(type: 'error', listener: () => void): void
  addEventListener(type: 'close', listener: (event: WsCloseEvent) => void): void
  close(): void
  send(data: string): void
}

type WebSocketConstructor = new (url: string) => WsLike

export interface RunAxonCommandWsStreamOptions {
  timeoutMs?: number
  input?: string
  flags?: Record<string, string | boolean>
  signal?: AbortSignal
  onJson?: (data: unknown) => void
  onOutputLine?: (line: string) => void
  onDone?: (payload: { exit_code: number; elapsed_ms?: number }) => void
  onError?: (payload: { message: string; elapsed_ms?: number }) => void
}

async function resolveWebSocketConstructor(): Promise<WebSocketConstructor> {
  const nativeConstructor = globalThis.WebSocket as unknown as WebSocketConstructor | undefined
  if (nativeConstructor) return nativeConstructor

  // Use dynamic module name to avoid type-check coupling to ws type declarations.
  const wsModuleName = 'ws'
  const wsModule = (await import(wsModuleName)) as {
    WebSocket?: WebSocketConstructor
    default?: WebSocketConstructor
  }
  if (wsModule.WebSocket) return wsModule.WebSocket
  if (wsModule.default) return wsModule.default

  throw new Error('WebSocket runtime is unavailable. Install ws or use Node.js 22+.')
}

/**
 * Run a synchronous axon command via the axon-workers WS bridge and return
 * the parsed JSON result. Rejects on timeout, connection error, or if the
 * command itself fails.
 */
export async function runAxonCommandWs(
  mode: string,
  timeoutMs = 30_000,
  input = '',
  flags: Record<string, string | boolean> = {},
): Promise<unknown> {
  let result: unknown
  let commandErrorMessage: string | null = null

  await runAxonCommandWsStream(mode, {
    timeoutMs,
    input,
    flags,
    onJson: (data) => {
      result = data
    },
    onError: (payload) => {
      commandErrorMessage = payload.message
    },
  })

  if (commandErrorMessage) {
    throw new Error(commandErrorMessage)
  }
  return result
}

export async function runAxonCommandWsStream(
  mode: string,
  options: RunAxonCommandWsStreamOptions = {},
): Promise<void> {
  const WebSocketImpl = await resolveWebSocketConstructor()
  const workersWsUrl = buildWorkersWsUrl()
  const timeoutMs = options.timeoutMs ?? 30_000
  const input = options.input ?? ''
  const flags = options.flags ?? {}
  const maxConnectAttempts = 4

  return new Promise((resolve, reject) => {
    let settled = false
    let ws: WsLike | null = null
    let opened = false
    let connectAttempts = 0
    const abortSignal = options.signal
    let timer: ReturnType<typeof setTimeout> | undefined

    const finish = (err?: Error) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      abortSignal?.removeEventListener('abort', onAbort)
      try {
        ws?.close()
      } catch {
        /* ignore */
      }
      if (err) reject(err)
      else resolve()
    }

    const onAbort = () => {
      finish(new Error(`axon ${mode} request aborted`))
    }

    if (abortSignal?.aborted) {
      onAbort()
      return
    }
    abortSignal?.addEventListener('abort', onAbort, { once: true })

    timer = setTimeout(
      () => finish(new Error(`Timeout waiting for axon ${mode} (${timeoutMs}ms)`)),
      timeoutMs,
    )

    const connect = () => {
      if (settled) return
      connectAttempts += 1
      ws = new WebSocketImpl(workersWsUrl)

      ws.addEventListener('open', () => {
        opened = true
        ws?.send(JSON.stringify({ type: 'execute', mode, input, flags }))
      })

      ws.addEventListener('message', (event) => {
        try {
          const parsed = JSON.parse(String(event.data)) as { type?: unknown; data?: unknown }
          const type = typeof parsed.type === 'string' ? parsed.type : ''
          const data =
            parsed.data && typeof parsed.data === 'object' && !Array.isArray(parsed.data)
              ? (parsed.data as Record<string, unknown>)
              : null

          if (type === 'command.output.json') {
            const outputData = data && data.data !== undefined ? data.data : data
            options.onJson?.(outputData)
            return
          }
          if (type === 'command.output.line') {
            options.onOutputLine?.(typeof data?.line === 'string' ? data.line : '')
            return
          }
          if (type === 'command.done') {
            const payload =
              data?.payload && typeof data.payload === 'object' && !Array.isArray(data.payload)
                ? (data.payload as Record<string, unknown>)
                : null
            options.onDone?.({
              exit_code: typeof payload?.exit_code === 'number' ? payload.exit_code : 0,
              elapsed_ms: typeof payload?.elapsed_ms === 'number' ? payload.elapsed_ms : undefined,
            })
            finish()
            return
          }
          if (type === 'command.error') {
            const payload =
              data?.payload && typeof data.payload === 'object' && !Array.isArray(data.payload)
                ? (data.payload as Record<string, unknown>)
                : null
            options.onError?.({
              message:
                typeof payload?.message === 'string' && payload.message.length > 0
                  ? payload.message
                  : `axon ${mode} failed`,
              elapsed_ms: typeof payload?.elapsed_ms === 'number' ? payload.elapsed_ms : undefined,
            })
            finish()
          }
        } catch {
          /* ignore non-JSON messages */
        }
      })

      ws.addEventListener('error', () => {
        if (!opened && connectAttempts < maxConnectAttempts) {
          setTimeout(connect, 250 * connectAttempts)
          return
        }
        finish(new Error(`WebSocket connection error (${WORKERS_WS_URL})`))
      })

      ws.addEventListener('close', (event) => {
        if (settled) return
        if (!opened && connectAttempts < maxConnectAttempts) {
          setTimeout(connect, 250 * connectAttempts)
          return
        }
        finish(new Error(`WebSocket closed unexpectedly (code ${event.code})`))
      })
    }

    connect()
  })
}
