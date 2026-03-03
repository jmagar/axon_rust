import { NextResponse } from 'next/server'
import { runAxonCommandWs } from '@/lib/axon-ws-exec'
import { apiError } from '@/lib/server/api-error'

export const dynamic = 'force-dynamic'

export async function GET() {
  try {
    const data = await runAxonCommandWs('domains', 60_000)
    return NextResponse.json({ ok: true, data })
  } catch (err) {
    console.error('[cortex/domains] failed', err)
    return apiError(500, 'Failed to fetch indexed domains', { code: 'cortex_domains' })
  }
}
