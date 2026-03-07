import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { runAxonCommandWsStream } from '@/lib/axon-ws-exec'

type MessageListener = (event: { data: unknown }) => void
type CloseListener = (event: { code: number }) => void
type OpenListener = () => void
type ErrorListener = () => void

class FakeWebSocket {
  static instances: FakeWebSocket[] = []

  private openListeners: OpenListener[] = []
  private messageListeners: MessageListener[] = []
  private errorListeners: ErrorListener[] = []
  private closeListeners: CloseListener[] = []

  sent: string[] = []

  constructor(_url: string) {
    FakeWebSocket.instances.push(this)
  }

  addEventListener(type: 'open', listener: OpenListener): void
  addEventListener(type: 'message', listener: MessageListener): void
  addEventListener(type: 'error', listener: ErrorListener): void
  addEventListener(type: 'close', listener: CloseListener): void
  addEventListener(
    type: 'open' | 'message' | 'error' | 'close',
    listener: OpenListener | MessageListener | ErrorListener | CloseListener,
  ): void {
    if (type === 'open') {
      this.openListeners.push(listener as OpenListener)
      return
    }
    if (type === 'message') {
      this.messageListeners.push(listener as MessageListener)
      return
    }
    if (type === 'error') {
      this.errorListeners.push(listener as ErrorListener)
      return
    }
    this.closeListeners.push(listener as CloseListener)
  }

  close(): void {
    // no-op for tests
  }

  send(data: string): void {
    this.sent.push(data)
  }

  emitOpen(): void {
    for (const listener of this.openListeners) {
      listener()
    }
  }

  emitMessage(data: unknown): void {
    for (const listener of this.messageListeners) {
      listener({ data })
    }
  }

  emitError(): void {
    for (const listener of this.errorListeners) {
      listener()
    }
  }

  emitClose(code = 1000): void {
    for (const listener of this.closeListeners) {
      listener({ code })
    }
  }
}

async function currentSocket(): Promise<FakeWebSocket> {
  for (let attempt = 0; attempt < 10; attempt += 1) {
    const socket = FakeWebSocket.instances.at(-1)
    if (socket) {
      return socket
    }
    await Promise.resolve()
  }
  throw new Error('expected FakeWebSocket instance to be created')
}

const originalWebSocket = globalThis.WebSocket

describe('runAxonCommandWsStream raw frame handling', () => {
  beforeEach(() => {
    FakeWebSocket.instances = []
    Object.defineProperty(globalThis, 'WebSocket', {
      configurable: true,
      writable: true,
      value: FakeWebSocket,
    })
  })

  afterEach(() => {
    vi.restoreAllMocks()
    if (originalWebSocket) {
      Object.defineProperty(globalThis, 'WebSocket', {
        configurable: true,
        writable: true,
        value: originalWebSocket,
      })
      return
    }
    // Keep global clean in runtimes without a native WebSocket.
    Reflect.deleteProperty(globalThis, 'WebSocket')
  })

  it('command.output.json callback receives payload from raw frame', async () => {
    const onJson = vi.fn()

    const stream = runAxonCommandWsStream('query', { timeoutMs: 1_000, onJson })
    const ws = await currentSocket()

    ws.emitOpen()
    ws.emitMessage(
      JSON.stringify({
        type: 'command.output.json',
        data: {
          ctx: { exec_id: 'e1', mode: 'query', input: 'hello' },
          data: { answer: 'ok', count: 2 },
        },
      }),
    )
    ws.emitMessage(
      JSON.stringify({
        type: 'command.done',
        data: { payload: { exit_code: 0, elapsed_ms: 12 } },
      }),
    )

    await expect(stream).resolves.toBeUndefined()
    expect(onJson).toHaveBeenCalledWith({ answer: 'ok', count: 2 })
  })

  it('command.done callback receives exit_code/elapsed_ms and resolves stream', async () => {
    const onDone = vi.fn()

    const stream = runAxonCommandWsStream('crawl', { timeoutMs: 1_000, onDone })
    const ws = await currentSocket()

    ws.emitOpen()
    ws.emitMessage(
      JSON.stringify({
        type: 'command.done',
        data: {
          payload: {
            exit_code: 0,
            elapsed_ms: 345,
          },
        },
      }),
    )

    await expect(stream).resolves.toBeUndefined()
    expect(onDone).toHaveBeenCalledWith({ exit_code: 0, elapsed_ms: 345 })
  })

  it('command.error callback receives message and resolves stream', async () => {
    const onError = vi.fn()

    const stream = runAxonCommandWsStream('extract', { timeoutMs: 1_000, onError })
    const ws = await currentSocket()

    ws.emitOpen()
    ws.emitMessage(
      JSON.stringify({
        type: 'command.error',
        data: {
          payload: {
            message: 'bad things happened',
            elapsed_ms: 88,
          },
        },
      }),
    )

    await expect(stream).resolves.toBeUndefined()
    expect(onError).toHaveBeenCalledWith({ message: 'bad things happened', elapsed_ms: 88 })
  })

  it('malformed/non-JSON WS frames are ignored without throwing', async () => {
    const onJson = vi.fn()

    const stream = runAxonCommandWsStream('query', { timeoutMs: 1_000, onJson })
    const ws = await currentSocket()

    ws.emitOpen()
    ws.emitMessage('not-json')
    ws.emitMessage({ impossible: 'to parse as json string' })
    ws.emitMessage('{"missing_type":true}')
    ws.emitMessage(
      JSON.stringify({
        type: 'command.done',
        data: { payload: { exit_code: 0, elapsed_ms: 5 } },
      }),
    )

    await expect(stream).resolves.toBeUndefined()
    expect(onJson).not.toHaveBeenCalled()
  })

  it('command.done with non-zero exit_code is surfaced to callback', async () => {
    const onDone = vi.fn()

    const stream = runAxonCommandWsStream('embed', { timeoutMs: 1_000, onDone })
    const ws = await currentSocket()

    ws.emitOpen()
    ws.emitMessage(
      JSON.stringify({
        type: 'command.done',
        data: {
          payload: {
            exit_code: 17,
            elapsed_ms: 901,
          },
        },
      }),
    )

    await expect(stream).resolves.toBeUndefined()
    expect(onDone).toHaveBeenCalledWith({ exit_code: 17, elapsed_ms: 901 })
  })
})
