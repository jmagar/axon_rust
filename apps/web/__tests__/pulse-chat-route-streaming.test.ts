import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type PulseChatStreamEvent, parsePulseChatStreamChunk } from '@/lib/pulse/chat-stream'

type MockChild = EventEmitter & {
  stdout: PassThrough
  stderr: PassThrough
  kill: ReturnType<typeof vi.fn>
}

type SpawnScenario = (child: MockChild) => void

let pendingScenario: SpawnScenario | null = null
let spawnSpy = vi.fn()

function makeMockChild(): MockChild {
  const child = new EventEmitter() as MockChild
  child.stdout = new PassThrough()
  child.stderr = new PassThrough()
  child.kill = vi.fn(() => {
    child.emit('close', null, 'SIGTERM')
  })
  return child
}

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

function queueScenario(scenario: SpawnScenario): void {
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
    spawnSpy = vi.fn()

    vi.doMock('node:child_process', () => ({
      spawn: (...args: unknown[]) => {
        spawnSpy(...args)
        if (!pendingScenario) {
          throw new Error('Missing spawn scenario for test')
        }
        const scenario = pendingScenario
        pendingScenario = null
        const child = makeMockChild()
        scenario(child)
        return child
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

  it('e2e-like: first assistant delta arrives before done even with malformed NDJSON lines', async () => {
    queueScenario((child) => {
      queueMicrotask(() => {
        child.stdout.write(
          `${JSON.stringify({ type: 'assistant', message: { content: [{ type: 'text', text: 'Hello from stream' }] } })}\n`,
        )
        child.stdout.write('not-json\n')
        child.stdout.write(
          `${JSON.stringify({ type: 'result', result: '{"text":"Final answer","operations":[]}' })}\n`,
        )
        child.emit('close', 0, null)
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
    expect(spawnSpy).toHaveBeenCalledTimes(1)
  })

  it('e2e-like: preserves partial assistant output when subprocess aborts before done', async () => {
    queueScenario((child) => {
      queueMicrotask(() => {
        child.stdout.write(
          `${JSON.stringify({ type: 'assistant', message: { content: [{ type: 'text', text: 'Partial output' }] } })}\n`,
        )
        child.emit('close', null, 'SIGTERM')
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
          code: 'pulse_chat_terminated_signal',
        }),
      ]),
    )
    expect(events.some((event) => event.type === 'done')).toBe(false)
  })

  it('e2e-like: emits status + tool_use events and includes tool results in done response blocks', async () => {
    queueScenario((child) => {
      queueMicrotask(() => {
        child.stdout.write(
          `${JSON.stringify({
            type: 'assistant',
            message: {
              content: [
                { type: 'text', text: 'Thinking...' },
                {
                  type: 'tool_use',
                  id: 'tool-1',
                  name: 'search_docs',
                  input: { query: 'rag' },
                },
              ],
            },
          })}\n`,
        )
        child.stdout.write(
          `${JSON.stringify({
            type: 'tool_result',
            tool_use_id: 'tool-1',
            content: [{ type: 'text', text: 'tool result text' }],
          })}\n`,
        )
        child.stdout.write(
          `${JSON.stringify({ type: 'result', result: '{"text":"Done","operations":[]}' })}\n`,
        )
        child.emit('close', 0, null)
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
    queueScenario((child) => {
      queueMicrotask(() => {
        child.stdout.write(
          `${JSON.stringify({ type: 'assistant', message: { content: [{ type: 'text', text: 'Replay me' }] } })}\n`,
        )
        child.stdout.write(
          `${JSON.stringify({ type: 'result', result: '{"text":"Replay done","operations":[]}' })}\n`,
        )
        child.emit('close', 0, null)
      })
    })

    const firstResponse = await post(makeRequest())
    const firstEvents = await readNdjsonEvents(firstResponse)
    const resumeFromId = firstEvents[0]?.event_id
    expect(typeof resumeFromId).toBe('string')
    expect(resumeFromId).toBeTruthy()

    // Do not queue another spawn scenario; replay should complete from cache and avoid spawning.
    const replayRequest = makeRequest({
      prompt: 'hello',
      last_event_id: resumeFromId,
    })
    const replayResponse = await post(replayRequest)
    const replayEvents = await readNdjsonEvents(replayResponse)

    expect(spawnSpy).toHaveBeenCalledTimes(1)
    expect(replayEvents.length).toBeGreaterThan(0)
    expect(replayEvents.some((event) => event.type === 'done')).toBe(true)
    expect(replayEvents.some((event) => event.type === 'assistant_delta')).toBe(true)
    expect(replayEvents[0]?.event_id).not.toBe(resumeFromId)
  })

  it('returns session_id from claude result event in done response', async () => {
    queueScenario((child) => {
      queueMicrotask(() => {
        child.stdout.write(
          `${JSON.stringify({ type: 'result', result: '{"text":"Answer","operations":[]}', session_id: 'abcdef01-abcd-1234-5678-abcdef012345' })}\n`,
        )
        child.emit('close', 0, null)
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

  it('passes --resume to claude spawn args when sessionId is in the request', async () => {
    queueScenario((child) => {
      queueMicrotask(() => {
        child.stdout.write(
          `${JSON.stringify({ type: 'result', result: '{"text":"Resumed","operations":[]}', session_id: 'deadbeef-cafe-1234-5678-abcdef012345' })}\n`,
        )
        child.emit('close', 0, null)
      })
    })

    await post(makeRequest({ prompt: 'hello', sessionId: 'deadbeef-cafe-1234-5678-abcdef012345' }))

    const spawnArgs: string[] = spawnSpy.mock.calls[0]?.[1] ?? []
    const resumeIdx = spawnArgs.indexOf('--resume')
    expect(resumeIdx).toBeGreaterThanOrEqual(0)
    expect(spawnArgs[resumeIdx + 1]).toBe('deadbeef-cafe-1234-5678-abcdef012345')
  })

  it('omits --resume from claude spawn args when sessionId is absent', async () => {
    queueScenario((child) => {
      queueMicrotask(() => {
        child.stdout.write(
          `${JSON.stringify({ type: 'result', result: '{"text":"Fresh","operations":[]}' })}\n`,
        )
        child.emit('close', 0, null)
      })
    })

    await post(makeRequest({ prompt: 'hello' }))

    const spawnArgs: string[] = spawnSpy.mock.calls[0]?.[1] ?? []
    expect(spawnArgs.includes('--resume')).toBe(false)
  })

  it('replays from a mid-stream event id through done (dropped-connection resume)', async () => {
    queueScenario((child) => {
      queueMicrotask(() => {
        child.stdout.write(
          `${JSON.stringify({
            type: 'assistant',
            message: {
              content: [
                { type: 'text', text: 'Chunk A' },
                { type: 'text', text: 'Chunk B' },
              ],
            },
          })}\n`,
        )
        child.stdout.write(
          `${JSON.stringify({ type: 'result', result: '{"text":"Reconnect complete","operations":[]}' })}\n`,
        )
        child.emit('close', 0, null)
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

    expect(spawnSpy).toHaveBeenCalledTimes(1)
    expect(replayEvents.length).toBeGreaterThan(0)
    expect(replayEvents.some((event) => event.type === 'done')).toBe(true)
    expect(replayEvents.some((event) => event.type === 'assistant_delta')).toBe(true)
    expect(replayEvents.every((event) => event.event_id !== midStreamEvent?.event_id)).toBe(true)
  })
})
