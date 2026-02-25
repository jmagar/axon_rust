import { NextResponse } from 'next/server'
import { CopilotRequestSchema } from '@/lib/pulse/copilot-validation'

export async function POST(request: Request) {
  const baseUrl = process.env.OPENAI_BASE_URL
  const apiKey = process.env.OPENAI_API_KEY
  const model = process.env.OPENAI_MODEL ?? 'gpt-4o-mini'

  if (!baseUrl || !apiKey) {
    return NextResponse.json(
      { error: 'OPENAI_BASE_URL and OPENAI_API_KEY must be set' },
      { status: 503 },
    )
  }

  const body = await request.json()
  const parsed = CopilotRequestSchema.safeParse(body)

  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.message }, { status: 400 })
  }

  const { prompt, system } = parsed.data
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
  })

  if (!response.ok) {
    const errText = await response.text()
    return NextResponse.json(
      { error: `LLM API error: ${response.status} ${errText}` },
      { status: 502 },
    )
  }

  const data = await response.json()
  const completion = data.choices?.[0]?.message?.content ?? ''

  return NextResponse.json({ completion })
}
