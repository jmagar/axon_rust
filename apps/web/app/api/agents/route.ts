import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { NextResponse } from 'next/server'
import { type Agent, parseAgentsOutput } from '@/lib/agents/parser'

export type { Agent } from '@/lib/agents/parser'

const execFileAsync = promisify(execFile)

interface AgentsResponse {
  agents: Agent[]
  groups: string[]
  error?: string
}

export async function GET(): Promise<NextResponse<AgentsResponse>> {
  try {
    const { stdout } = await execFileAsync('claude', ['agents'], { timeout: 10000 })
    const { agents, groups } = parseAgentsOutput(stdout)
    return NextResponse.json({ agents, groups })
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    return NextResponse.json({ agents: [], groups: [], error: message })
  }
}
