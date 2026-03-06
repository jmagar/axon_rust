import fs from 'node:fs/promises'
import { NextResponse } from 'next/server'
import { parseClaudeJsonl } from '@/lib/sessions/claude-jsonl-parser'
import { scanSessions } from '@/lib/sessions/session-scanner'

export async function GET(_request: Request, { params }: { params: Promise<{ id: string }> }) {
  const { id } = await params
  const sessions = await scanSessions(200)
  const session = sessions.find((s) => s.id === id)
  if (!session) {
    return NextResponse.json({ error: 'not found' }, { status: 404 })
  }

  try {
    const raw = await fs.readFile(session.absolutePath, 'utf-8')
    const messages = parseClaudeJsonl(raw)
    return NextResponse.json({
      project: session.project,
      filename: session.filename,
      sessionId: session.id,
      messages,
    })
  } catch {
    return NextResponse.json({ error: 'read failed' }, { status: 500 })
  }
}
