import { NextResponse } from 'next/server'
import { runAxonCommandWs } from '@/lib/axon-ws-exec'
import { apiError } from '@/lib/server/api-error'

export const dynamic = 'force-dynamic'

export async function GET() {
  try {
    const data = await runAxonCommandWs('stats', 30_000)
    return NextResponse.json({ ok: true, data })
  } catch (err) {
    console.error('[cortex/stats] failed', err)
    return apiError(500, 'Failed to fetch Cortex stats', { code: 'cortex_stats' })
  }
}
