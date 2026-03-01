import { execFile } from 'node:child_process'
import path from 'node:path'
import { promisify } from 'node:util'
import { NextResponse } from 'next/server'
import { ensureRepoRootEnvLoaded } from '@/lib/pulse/server-env'
import { getWorkspaceRoot } from '@/lib/pulse/workspace-root'

const execFileAsync = promisify(execFile)
export const dynamic = 'force-dynamic'

export async function GET() {
  ensureRepoRootEnvLoaded()
  const root = getWorkspaceRoot()
  const bin = path.join(root, 'scripts', 'axon')
  try {
    const { stdout } = await execFileAsync(bin, ['stats', '--json'], {
      timeout: 30_000,
      env: process.env,
      cwd: root,
    })
    const data = JSON.parse(stdout.trim())
    return NextResponse.json({ ok: true, data })
  } catch (err) {
    return NextResponse.json({ ok: false, error: String(err) }, { status: 500 })
  }
}
