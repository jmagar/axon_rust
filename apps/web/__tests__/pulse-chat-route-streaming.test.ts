import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { RunAxonCommandWsStreamOptions } from '@/lib/axon-ws-exec'
import { type PulseChatStreamEvent, parsePulseChatStreamChunk } from '@/lib/pulse/chat-stream'

type WsScenario = (args: {
  mode: string
  options: RunAxonCommandWsStreamOptions
}) => void | Promise<void>

let pendingScenario: WsScenario | null = null
let wsRunSpy = vi.fn()

async function readNdjsonEvents(response: Response): Promise<PulseChatStreamEvent[]> {
  if (!response.body) return []

  const decoder = new TextDecoder()
  const reader = response.body.getReader()
  const events: PulseChatStreamEvent[] = []
  let remainder = ''

  while (true) {
    const { done, value } = await reader.read()
    if (done) break

    const chunk = decoder.decode(value, { stream: true })
    const parsed = parsePulseChatStreamChunk(chunk, remainder)
    events.push(...parsed.events)
    remainder = parsed.remainder
  }

  const tail = parsePulseChatStreamChunk('\n', remainder)
  events.push(...tail.events)
  return events
}

function queueScenario(scenario: WsScenario): void {
  pendingScenario = scenario
}

function makeRequest(payload: Record<string, unknown> = { prompt: 'hello' }): Request {
  return new Request('http://localhost/api/pulse/chat', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  })
}

describe('pulse chat route streaming (e2e-like via Vitest; browser e2e harness unavailable)', () => {
  let post: (request: Request) => Promise<Response>

  beforeEach(async () => {
    vi.resetModules()
    pendingScenario = null
    wsRunSpy = vi.fn()

    vi.doMock('@/lib/axon-ws-exec', () => ({
      runAxonCommandWsStream: (
        mode: string,
        options: RunAxonCommandWsStreamOptions = {},
      ): Promise<void> => {
        wsRunSpy(mode, options)
        if (!pendingScenario) {
          throw new Error('Missing WS scenario for test')
        }
        const scenario = pendingScenario
        pendingScenario = null
        return Promise.resolve(scenario({ mode, options }))
      },
    }))

    vi.doMock('@/lib/pulse/rag', () => ({
      retrieveFromCollections: vi.fn(async () => []),
      buildPulseSystemPrompt: vi.fn(() => 'system prompt'),
    }))
    vi.doMock('@/lib/pulse/server-env', () => ({
      ensureRepoRootEnvLoaded: vi.fn(),
    }))
    vi.doMock('@/lib/pulse/permissions', () => ({
      checkPermission: vi.fn(() => ({ allowed: true })),
    }))
    vi.doMock('@/lib/pulse/conversation-memory', () => ({
      resolveConversationMemoryAnswer: vi.fn(() => null),
    }))

    const routeModule = await import('@/app/api/pulse/chat/route')
    post = routeModule.POST
  })

  it('e2e-like: first assistant delta arrives before done via WS output frames', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({ type: 'assistant_delta', delta: 'Hello from stream' })
        options.onJson?.({ type: 'result', result: '{"text":"Final answer","operations":[]}' })
        options.onDone?.({ exit_code: 0 })
      })
    })

    const response = await post(makeRequest())
    const events = await readNdjsonEvents(response)

    const firstDeltaIndex = events.findIndex((event) => event.type === 'assistant_delta')
    const doneIndex = events.findIndex((event) => event.type === 'done')

    expect(firstDeltaIndex).toBeGreaterThanOrEqual(0)
    expect(doneIndex).toBeGreaterThan(firstDeltaIndex)

    const doneEvent = events.find((event) => event.type === 'done')
    expect(doneEvent).toMatchObject({
      type: 'done',
      response: {
        text: 'Final answer',
      },
    })
    expect(wsRunSpy).toHaveBeenCalledTimes(1)
    expect(wsRunSpy.mock.calls[0]?.[0]).toBe('pulse_chat')
  })

  it('e2e-like: preserves partial output and emits error when worker sends command.error', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({ type: 'assistant_delta', delta: 'Partial output' })
        options.onError?.({ message: 'worker crashed' })
      })
    })

    const response = await post(makeRequest())
    const events = await readNdjsonEvents(response)

    expect(events).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'assistant_delta',
          delta: 'Partial output',
        }),
        expect.objectContaining({
          type: 'error',
          code: 'pulse_chat_command_error',
        }),
      ]),
    )
    expect(events.some((event) => event.type === 'done')).toBe(false)
  })

  it('e2e-like: emits error when worker exits non-zero via command.done', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({ type: 'assistant_delta', delta: 'Partial output' })
        options.onDone?.({ exit_code: 2 })
      })
    })

    const response = await post(makeRequest())
    const events = await readNdjsonEvents(response)

    expect(events).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'assistant_delta',
          delta: 'Partial output',
        }),
        expect.objectContaining({
          type: 'error',
          code: 'pulse_chat_exit_nonzero',
        }),
      ]),
    )
    expect(events.some((event) => event.type === 'done')).toBe(false)
  })

  it('e2e-like: emits status + tool_use events and includes tool results in done response blocks', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({ type: 'status', phase: 'thinking' })
        options.onJson?.({ type: 'assistant_delta', delta: 'Thinking...' })
        options.onJson?.({
          type: 'tool_use',
          tool_call_id: 'tool-1',
          tool: { name: 'search_docs', input: { query: 'rag' } },
        })
        options.onJson?.({
          type: 'tool_result',
          tool_call_id: 'tool-1',
          content: [{ type: 'text', text: 'tool result text' }],
        })
        options.onJson?.({ type: 'result', result: '{"text":"Done","operations":[]}' })
        options.onDone?.({ exit_code: 0 })
      })
    })

    const response = await post(makeRequest())
    const events = await readNdjsonEvents(response)

    expect(events[0]).toMatchObject({ type: 'status', phase: 'started' })
    expect(events).toEqual(
      expect.arrayContaining([expect.objectContaining({ type: 'status', phase: 'thinking' })]),
    )
    expect(events).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'tool_use',
          tool: { name: 'search_docs', input: { query: 'rag' } },
        }),
      ]),
    )
    expect(events).toEqual(
      expect.arrayContaining([expect.objectContaining({ type: 'status', phase: 'finalizing' })]),
    )

    const doneEvent = events.find((event) => event.type === 'done')
    expect(doneEvent).toMatchObject({
      type: 'done',
      response: {
        text: 'Done',
        toolUses: [{ name: 'search_docs', input: { query: 'rag' } }],
        blocks: [
          { type: 'text', content: 'Thinking...' },
          {
            type: 'tool_use',
            name: 'search_docs',
            input: { query: 'rag' },
            result: 'tool result text',
          },
        ],
      },
    })
  })

  it('replays cached tail events when last_event_id is provided', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({ type: 'assistant_delta', delta: 'Replay me' })
        options.onJson?.({ type: 'result', result: '{"text":"Replay done","operations":[]}' })
        options.onDone?.({ exit_code: 0 })
      })
    })

    const firstResponse = await post(makeRequest())
    const firstEvents = await readNdjsonEvents(firstResponse)
    const resumeFromId = firstEvents[0]?.event_id
    expect(typeof resumeFromId).toBe('string')
    expect(resumeFromId).toBeTruthy()

    const replayRequest = makeRequest({
      prompt: 'hello',
      last_event_id: resumeFromId,
    })
    const replayResponse = await post(replayRequest)
    const replayEvents = await readNdjsonEvents(replayResponse)

    expect(wsRunSpy).toHaveBeenCalledTimes(1)
    expect(replayEvents.length).toBeGreaterThan(0)
    expect(replayEvents.some((event) => event.type === 'done')).toBe(true)
    expect(replayEvents.some((event) => event.type === 'assistant_delta')).toBe(true)
    expect(replayEvents[0]?.event_id).not.toBe(resumeFromId)
  })

  it('returns session_id from result event in done response', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({
          type: 'result',
          result: '{"text":"Answer","operations":[]}',
          session_id: 'abcdef01-abcd-1234-5678-abcdef012345',
        })
        options.onDone?.({ exit_code: 0 })
      })
    })

    const response = await post(makeRequest())
    const events = await readNdjsonEvents(response)

    const doneEvent = events.find((event) => event.type === 'done')
    expect(doneEvent).toMatchObject({
      type: 'done',
      response: {
        text: 'Answer',
        sessionId: 'abcdef01-abcd-1234-5678-abcdef012345',
      },
    })
  })

  it('passes session_id flag to pulse_chat WS mode when sessionId is provided', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({
          type: 'result',
          result: '{"text":"Resumed","operations":[]}',
          session_id: 'deadbeef-cafe-1234-5678-abcdef012345',
        })
        options.onDone?.({ exit_code: 0 })
      })
    })

    await post(makeRequest({ prompt: 'hello', sessionId: 'deadbeef-cafe-1234-5678-abcdef012345' }))

    const wsOptions = wsRunSpy.mock.calls[0]?.[1] as RunAxonCommandWsStreamOptions
    expect(wsOptions.flags?.session_id).toBe('deadbeef-cafe-1234-5678-abcdef012345')
  })

  it('omits session_id WS flag when sessionId is absent', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({ type: 'result', result: '{"text":"Fresh","operations":[]}' })
        options.onDone?.({ exit_code: 0 })
      })
    })

    await post(makeRequest({ prompt: 'hello' }))

    const wsOptions = wsRunSpy.mock.calls[0]?.[1] as RunAxonCommandWsStreamOptions
    expect(wsOptions.flags?.session_id).toBeUndefined()
  })

  it('passes agent flag to pulse_chat WS mode', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({ type: 'result', result: '{"text":"Codex","operations":[]}' })
        options.onDone?.({ exit_code: 0 })
      })
    })

    await post(makeRequest({ prompt: 'hello', agent: 'codex' }))

    const wsOptions = wsRunSpy.mock.calls[0]?.[1] as RunAxonCommandWsStreamOptions
    expect(wsOptions.flags?.agent).toBe('codex')
  })

  it('passes model flag to pulse_chat WS mode', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({ type: 'result', result: '{"text":"Model","operations":[]}' })
        options.onDone?.({ exit_code: 0 })
      })
    })

    await post(makeRequest({ prompt: 'hello', agent: 'codex', model: 'o3' }))

    const wsOptions = wsRunSpy.mock.calls[0]?.[1] as RunAxonCommandWsStreamOptions
    expect(wsOptions.flags?.model).toBe('o3')
  })

  it('omits default model flag for codex agent', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({ type: 'result', result: '{"text":"Default model","operations":[]}' })
        options.onDone?.({ exit_code: 0 })
      })
    })

    await post(makeRequest({ prompt: 'hello', agent: 'codex', model: 'default' }))

    const wsOptions = wsRunSpy.mock.calls[0]?.[1] as RunAxonCommandWsStreamOptions
    expect(wsOptions.flags?.model).toBeUndefined()
  })

  it('normalizes singular config_option_update events from ACP', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({
          type: 'config_option_update',
          configOptions: [
            {
              id: 'model',
              name: 'Model',
              category: 'model',
              currentValue: 'gpt-5.3-codex',
              options: [{ value: 'gpt-5.3-codex', name: 'GPT 5.3 Codex' }],
            },
          ],
        })
        options.onJson?.({ type: 'result', result: '{"text":"OK","operations":[]}' })
        options.onDone?.({ exit_code: 0 })
      })
    })

    const response = await post(makeRequest())
    const events = await readNdjsonEvents(response)

    expect(events).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'config_options_update',
          configOptions: [
            expect.objectContaining({
              id: 'model',
              currentValue: 'gpt-5.3-codex',
            }),
          ],
        }),
      ]),
    )
  })

  it('replays from a mid-stream event id through done (dropped-connection resume)', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({ type: 'assistant_delta', delta: 'Chunk A' })
        options.onJson?.({ type: 'assistant_delta', delta: 'Chunk B' })
        options.onJson?.({
          type: 'result',
          result: '{"text":"Reconnect complete","operations":[]}',
        })
        options.onDone?.({ exit_code: 0 })
      })
    })

    const initialResponse = await post(makeRequest())
    const initialEvents = await readNdjsonEvents(initialResponse)
    const midStreamEvent = initialEvents.find((event) => event.type === 'assistant_delta')
    expect(midStreamEvent?.event_id).toBeTruthy()

    const replayResponse = await post(
      makeRequest({
        prompt: 'hello',
        last_event_id: midStreamEvent?.event_id,
      }),
    )
    const replayEvents = await readNdjsonEvents(replayResponse)

    expect(wsRunSpy).toHaveBeenCalledTimes(1)
    expect(replayEvents.length).toBeGreaterThan(0)
    expect(replayEvents.some((event) => event.type === 'done')).toBe(true)
    expect(replayEvents.some((event) => event.type === 'assistant_delta')).toBe(true)
    expect(replayEvents.every((event) => event.event_id !== midStreamEvent?.event_id)).toBe(true)
  })
})
