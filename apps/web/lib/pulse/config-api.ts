import { apiFetch } from '@/lib/api-fetch'
import { AcpConfigOption, type PulseAgent, type PulseModel } from '@/lib/pulse/types'

interface ProbePulseConfigOptionsInput {
  agent: PulseAgent
  sessionId?: string | null
  model?: PulseModel
}

export async function probePulseConfigOptions({
  agent,
  sessionId,
  model,
}: ProbePulseConfigOptionsInput): Promise<AcpConfigOption[]> {
  const response = await apiFetch('/api/pulse/config', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      agent,
      sessionId: sessionId ?? undefined,
      model: model ?? undefined,
    }),
  })

  if (!response.ok) {
    const body = await response.text()
    throw new Error(body || `Pulse config probe failed (${response.status})`)
  }

  const parsed = (await response.json()) as { configOptions?: unknown }
  return AcpConfigOption.array().parse(parsed.configOptions ?? [])
}
