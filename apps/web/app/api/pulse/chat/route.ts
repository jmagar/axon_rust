import { NextResponse } from 'next/server'
import { validateDocOperations } from '@/lib/pulse/doc-ops'
import { checkPermission } from '@/lib/pulse/permissions'
import { buildPulseSystemPrompt, retrieveFromCollections } from '@/lib/pulse/rag'
import { ensureRepoRootEnvLoaded } from '@/lib/pulse/server-env'
import {
  DocOperationSchema,
  PulseChatRequestSchema,
  type PulseChatResponse,
} from '@/lib/pulse/types'

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

  const body = await request.json()
  const parsed = PulseChatRequestSchema.safeParse(body)
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.message }, { status: 400 })
  }

  const req = parsed.data
  const citations = await retrieveFromCollections(req.prompt, req.selectedCollections, 4)
  const systemPrompt = buildPulseSystemPrompt(req, citations)

  const completionRes = await fetch(`${baseUrl}/chat/completions`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${apiKey}`,
    },
    body: JSON.stringify({
      model,
      temperature: 0.2,
      messages: [
        { role: 'system' as const, content: systemPrompt },
        ...req.conversationHistory.map((m) => ({ role: m.role, content: m.content })),
        {
          role: 'user' as const,
          content: [
            req.prompt,
            '',
            'Respond as JSON with shape:',
            '{"text":"...", "operations":[...]}',
            'Use operation types only: replace_document, append_markdown, insert_section.',
          ].join('\n'),
        },
      ],
      response_format: { type: 'json_object' },
    }),
  })

  if (!completionRes.ok) {
    const errText = await completionRes.text()
    return NextResponse.json(
      { error: `LLM API error: ${completionRes.status} ${errText}` },
      { status: 502 },
    )
  }

  const completionJson = await completionRes.json()
  const raw = completionJson.choices?.[0]?.message?.content ?? '{}'

  let text = ''
  let operations: PulseChatResponse['operations'] = []
  try {
    const parsedJson = JSON.parse(raw)
    text = String(parsedJson.text ?? '')
    if (Array.isArray(parsedJson.operations)) {
      const parsedOps: PulseChatResponse['operations'] = []
      for (const op of parsedJson.operations as unknown[]) {
        const parsedOp = DocOperationSchema.safeParse(op)
        if (parsedOp.success) {
          parsedOps.push(parsedOp.data)
        }
      }
      operations = parsedOps
    } else {
      operations = []
    }
  } catch {
    text = raw
  }

  const permission = checkPermission(req.permissionLevel, operations, {
    isCurrentDoc: true,
    currentDocMarkdown: req.documentMarkdown,
  })

  if (!permission.allowed) {
    operations = []
    text = text || 'Operation blocked by permission policy.'
  }

  if (operations.length > 0) {
    const validation = validateDocOperations(operations, req.documentMarkdown)
    if (!validation.valid && req.permissionLevel === 'plan') {
      operations = []
    }
  }

  return NextResponse.json({ text, citations, operations } satisfies PulseChatResponse)
}
