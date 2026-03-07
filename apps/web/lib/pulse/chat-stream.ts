import type { AcpConfigOption, PulseChatResponse, PulseToolUse } from '@/lib/pulse/types'

export const PULSE_CHAT_STREAM_PROTOCOL_VERSION = 1 as const

type PulseChatStreamEventPayload =
  | { type: 'status'; phase: 'started' | 'thinking' | 'finalizing' }
  | { type: 'assistant_delta'; delta: string }
  | { type: 'thinking_content'; content: string }
  | { type: 'tool_use'; tool: PulseToolUse }
  | { type: 'config_options_update'; configOptions: AcpConfigOption[] }
  | { type: 'heartbeat'; elapsed_ms: number }
  | { type: 'done'; response: PulseChatResponse }
  | { type: 'error'; error: string; code?: string }

export type PulseChatStreamEvent = PulseChatStreamEventPayload & {
  protocol_version: number
  event_id: string
}

function createEventId(): string {
  return crypto.randomUUID()
}

export function createPulseChatStreamEvent(
  event: PulseChatStreamEvent | PulseChatStreamEventPayload,
): PulseChatStreamEvent {
  if ('protocol_version' in event && 'event_id' in event) {
    return event
  }

  return {
    ...event,
    protocol_version: PULSE_CHAT_STREAM_PROTOCOL_VERSION,
    event_id: createEventId(),
  }
}

export function encodePulseChatStreamEvent(
  event: PulseChatStreamEvent | PulseChatStreamEventPayload,
): string {
  return `${JSON.stringify(createPulseChatStreamEvent(event))}\n`
}

export function parsePulseChatStreamChunk(
  chunk: string,
  remainder: string,
): { events: PulseChatStreamEvent[]; remainder: string } {
  const combined = remainder + chunk
  const lines = combined.split('\n')
  const nextRemainder = lines.pop() ?? ''
  const events: PulseChatStreamEvent[] = []

  for (const line of lines) {
    const trimmed = line.trim()
    if (!trimmed) continue
    try {
      const parsed = JSON.parse(trimmed) as Partial<PulseChatStreamEvent> & { type?: string }
      if (!parsed || typeof parsed !== 'object' || typeof parsed.type !== 'string') continue
      const withProtocol =
        typeof parsed.protocol_version === 'number' && typeof parsed.event_id === 'string'
          ? parsed
          : {
              ...parsed,
              protocol_version: PULSE_CHAT_STREAM_PROTOCOL_VERSION,
              event_id: createEventId(),
            }
      events.push(withProtocol as PulseChatStreamEvent)
    } catch {
      // Ignore malformed NDJSON lines from interrupted chunks.
    }
  }

  return { events, remainder: nextRemainder }
}
