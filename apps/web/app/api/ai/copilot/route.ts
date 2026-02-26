import { NextResponse } from 'next/server'
import type { z } from 'zod'
import { CopilotRequestSchema } from '@/lib/pulse/copilot-validation'
import { ensureRepoRootEnvLoaded } from '@/lib/pulse/server-env'

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
      }),
      signal: controller.signal,
    }).finally(() => clearTimeout(timeout))

    if (!response.ok) {
      return NextResponse.json({ error: `LLM API error: ${response.status}` }, { status: 502 })
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
