import { NextResponse } from 'next/server'
import type { z } from 'zod'
import { CopilotRequestSchema } from '@/lib/pulse/copilot-validation'
import { ensureRepoRootEnvLoaded } from '@/lib/pulse/server-env'

type OpenAiSseParseResult = {
  deltas: string[]
  remainder: string
  done: boolean
}

type CopilotStreamEvent =
  | { type: 'start' }
  | { type: 'delta'; delta: string; completion: string }
  | { type: 'done'; completion: string }
  | { type: 'error'; error: string }

export async function POST(request: Request) {
  ensureRepoRootEnvLoaded()

  const baseUrl = process.env.OPENAI_BASE_URL
  const apiKey = process.env.OPENAI_API_KEY
  const model = process.env.OPENAI_MODEL ?? 'gpt-4o-mini'

  if (!baseUrl || !apiKey) {
    const missing = [...(baseUrl ? [] : ['OPENAI_BASE_URL']), ...(apiKey ? [] : ['OPENAI_API_KEY'])]
    return NextResponse.json(
      {
        error: `${missing.join(', ')} must be set`,
        missing,
        hint: 'Set these in apps/web/.env.local (or export them before starting next dev).',
      },
      { status: 503 },
    )
  }

  try {
    const body = await request.json()
    const parsed = CopilotRequestSchema.safeParse(body)

    if (!parsed.success) {
      return NextResponse.json({ error: firstZodIssue(parsed.error) }, { status: 400 })
    }

    const wantsNdjsonStream = requestWantsNdjsonStream(request)
    const { prompt, system } = parsed.data
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), 20_000)
    const response = await fetch(`${baseUrl}/chat/completions`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${apiKey}`,
      },
      body: JSON.stringify({
        model: parsed.data.model ?? model,
        messages: [
          ...(system ? [{ role: 'system' as const, content: system }] : []),
          { role: 'user' as const, content: prompt },
        ],
        max_tokens: 200,
        temperature: 0.7,
        ...(wantsNdjsonStream ? { stream: true } : {}),
      }),
      signal: controller.signal,
    }).finally(() => clearTimeout(timeout))

    if (!response.ok) {
      return NextResponse.json({ error: `LLM API error: ${response.status}` }, { status: 502 })
    }

    if (wantsNdjsonStream) {
      return streamNdjsonCompletion(response)
    }

    const data = await response.json()
    const completion = data.choices?.[0]?.message?.content ?? ''

    return NextResponse.json({ completion })
  } catch (err) {
    console.error('[Copilot] Unhandled error:', err)
    return NextResponse.json({ error: 'Copilot request failed' }, { status: 500 })
  }
}

function firstZodIssue(error: z.ZodError): string {
  return error.issues[0]?.message ?? 'Invalid request payload'
}

function requestWantsNdjsonStream(request: Request): boolean {
  const accept = request.headers.get('accept')?.toLowerCase() ?? ''
  const streamHeader = request.headers.get('x-copilot-stream')?.toLowerCase()
  return accept.includes('application/x-ndjson') || streamHeader === '1' || streamHeader === 'true'
}

function streamNdjsonCompletion(upstream: Response): Response {
  const contentType = upstream.headers.get('content-type')?.toLowerCase() ?? ''

  if (contentType.includes('application/json')) {
    return new Response(
      new ReadableStream({
        async start(controller) {
          try {
            const json = await upstream.json()
            const completion = json?.choices?.[0]?.message?.content ?? ''
            controller.enqueue(encodeCopilotStreamEvent({ type: 'start' }))
            if (completion) {
              controller.enqueue(
                encodeCopilotStreamEvent({
                  type: 'delta',
                  delta: completion,
                  completion,
                }),
              )
            }
            controller.enqueue(encodeCopilotStreamEvent({ type: 'done', completion }))
          } catch {
            controller.enqueue(
              encodeCopilotStreamEvent({ type: 'error', error: 'Invalid upstream JSON response' }),
            )
          } finally {
            controller.close()
          }
        },
      }),
      {
        headers: {
          'Content-Type': 'application/x-ndjson; charset=utf-8',
          'Cache-Control': 'no-store',
        },
      },
    )
  }

  return new Response(
    new ReadableStream({
      async start(controller) {
        controller.enqueue(encodeCopilotStreamEvent({ type: 'start' }))

        if (!upstream.body) {
          controller.enqueue(encodeCopilotStreamEvent({ type: 'done', completion: '' }))
          controller.close()
          return
        }

        const reader = upstream.body.getReader()
        const decoder = new TextDecoder()
        let remainder = ''
        let completion = ''

        try {
          while (true) {
            const { value, done } = await reader.read()
            if (done) break

            const parsed = parseOpenAiSseChunk(decoder.decode(value, { stream: true }), remainder)
            remainder = parsed.remainder

            for (const delta of parsed.deltas) {
              completion += delta
              controller.enqueue(encodeCopilotStreamEvent({ type: 'delta', delta, completion }))
            }

            if (parsed.done) break
          }

          controller.enqueue(encodeCopilotStreamEvent({ type: 'done', completion }))
        } catch {
          controller.enqueue(
            encodeCopilotStreamEvent({ type: 'error', error: 'Streaming completion failed' }),
          )
        } finally {
          controller.close()
        }
      },
    }),
    {
      headers: {
        'Content-Type': 'application/x-ndjson; charset=utf-8',
        'Cache-Control': 'no-store',
      },
    },
  )
}

export function encodeCopilotStreamEvent(event: CopilotStreamEvent): string {
  return `${JSON.stringify(event)}\n`
}

export function parseOpenAiSseChunk(chunk: string, remainder: string): OpenAiSseParseResult {
  const combined = remainder + chunk
  const lines = combined.split('\n')
  const nextRemainder = lines.pop() ?? ''
  const deltas: string[] = []
  let done = false

  for (const rawLine of lines) {
    const line = rawLine.trim()
    if (!line || line.startsWith(':') || !line.startsWith('data:')) continue

    const payload = line.slice(5).trim()
    if (!payload) continue
    if (payload === '[DONE]') {
      done = true
      break
    }

    try {
      const parsed = JSON.parse(payload) as {
        choices?: Array<{ delta?: { content?: string } }>
      }
      const delta = parsed.choices?.[0]?.delta?.content
      if (typeof delta === 'string' && delta.length > 0) {
        deltas.push(delta)
      }
    } catch {
      // Ignore malformed lines and continue parsing the stream.
    }
  }

  return { deltas, remainder: nextRemainder, done }
}
