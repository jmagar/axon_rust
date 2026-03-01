import { describe, expect, it } from 'vitest'
import { encodePulseChatStreamEvent, parsePulseChatStreamChunk } from '@/lib/pulse/chat-stream'

describe('pulse chat stream parser', () => {
  it('parses NDJSON events from chunk boundaries', () => {
    const eventA = encodePulseChatStreamEvent({
      type: 'assistant_delta',
      delta: 'Hello ',
    })
    const eventB = encodePulseChatStreamEvent({
      type: 'assistant_delta',
      delta: 'world',
    })
    const merged = `${eventA}${eventB}`

    const firstHalf = merged.slice(0, eventA.length + 3)
    const secondHalf = merged.slice(eventA.length + 3)

    const firstPass = parsePulseChatStreamChunk(firstHalf, '')
    expect(firstPass.events).toHaveLength(1)
    expect(firstPass.events[0]).toMatchObject({
      type: 'assistant_delta',
      delta: 'Hello ',
    })

    const secondPass = parsePulseChatStreamChunk(secondHalf, firstPass.remainder)
    expect(secondPass.events).toHaveLength(1)
    expect(secondPass.events[0]).toMatchObject({
      type: 'assistant_delta',
      delta: 'world',
    })
    expect(secondPass.remainder).toBe('')
  })

  it('ignores malformed lines and preserves trailing partial line', () => {
    const valid = encodePulseChatStreamEvent({
      type: 'status',
      phase: 'thinking',
    })
    const malformed = '{"type":'
    const mixed = `not-json\n${valid}${malformed}`

    const parsed = parsePulseChatStreamChunk(mixed, '')
    expect(parsed.events).toHaveLength(1)
    expect(parsed.events[0]).toMatchObject({
      type: 'status',
      phase: 'thinking',
    })
    expect(parsed.remainder).toBe(malformed)
  })

  it('tolerates malformed complete lines between valid NDJSON events', () => {
    const eventA = encodePulseChatStreamEvent({
      type: 'assistant_delta',
      delta: 'alpha',
    })
    const eventB = encodePulseChatStreamEvent({
      type: 'assistant_delta',
      delta: 'omega',
    })
    const mixed = `${eventA}{"type":\n{"unexpected":"shape"}\n${eventB}`

    const parsed = parsePulseChatStreamChunk(mixed, '')
    expect(parsed.events).toHaveLength(2)
    expect(parsed.events[0]).toMatchObject({
      type: 'assistant_delta',
      delta: 'alpha',
    })
    expect(parsed.events[1]).toMatchObject({
      type: 'assistant_delta',
      delta: 'omega',
    })
    expect(parsed.remainder).toBe('')
  })

  it('backfills protocol metadata for legacy events missing ids', () => {
    const legacy = '{"type":"assistant_delta","delta":"hello"}\n'
    const parsed = parsePulseChatStreamChunk(legacy, '')

    expect(parsed.events).toHaveLength(1)
    expect(parsed.events[0]).toMatchObject({
      type: 'assistant_delta',
      delta: 'hello',
      protocol_version: 1,
    })
    expect(typeof parsed.events[0].event_id).toBe('string')
    expect(parsed.events[0].event_id.length).toBeGreaterThan(0)
  })

  it('preserves existing protocol metadata when present', () => {
    const withMeta =
      '{"type":"status","phase":"started","protocol_version":42,"event_id":"evt-123"}\n'

    const parsed = parsePulseChatStreamChunk(withMeta, '')
    expect(parsed.events).toHaveLength(1)
    expect(parsed.events[0]).toMatchObject({
      type: 'status',
      phase: 'started',
      protocol_version: 42,
      event_id: 'evt-123',
    })
  })
})
