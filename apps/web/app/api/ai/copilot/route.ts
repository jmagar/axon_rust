import { generateText } from 'ai'
import type { NextRequest } from 'next/server'
import { NextResponse } from 'next/server'

/** Parse OpenAI SSE streaming chunks — re-exported for use by other routes. */
export function parseOpenAiSseChunk(
  chunk: string,
  remainder: string,
): { deltas: string[]; done: boolean; remainder: string } {
  const combined = remainder + chunk
  const lines = combined.split('\n')
  const nextRemainder = lines.pop() ?? ''
  const deltas: string[] = []
  let done = false

  for (const rawLine of lines) {
    const line = rawLine.trim()
    if (!line || !line.startsWith('data:')) continue
    const data = line.slice('data:'.length).trim()
    if (data === '[DONE]') {
      done = true
      continue
    }
    try {
      const parsed = JSON.parse(data)
      const delta = parsed?.choices?.[0]?.delta?.content
      if (typeof delta === 'string' && delta.length > 0) {
        deltas.push(delta)
      }
    } catch {
      // Ignore malformed SSE lines.
    }
  }

  return { deltas, done, remainder: nextRemainder }
}

export async function POST(req: NextRequest) {
  const { apiKey: key, model = 'gpt-4o-mini', prompt, system } = await req.json()

  const apiKey = key || process.env.AI_GATEWAY_API_KEY

  if (!apiKey) {
    return NextResponse.json({ error: 'Missing ai gateway API key.' }, { status: 401 })
  }

  try {
    const result = await generateText({
      abortSignal: req.signal,
      maxOutputTokens: 50,
      model: `openai/${model}`,
      prompt,
      system,
      temperature: 0.7,
    })

    return NextResponse.json(result)
  } catch (error) {
    if (error instanceof Error && error.name === 'AbortError') {
      return NextResponse.json(null, { status: 408 })
    }

    return NextResponse.json({ error: 'Failed to process AI request' }, { status: 500 })
  }
}
