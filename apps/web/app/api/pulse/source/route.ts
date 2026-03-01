import { spawn } from 'node:child_process'
import path from 'node:path'
import { NextResponse } from 'next/server'
import { ensureRepoRootEnvLoaded } from '@/lib/pulse/server-env'
import { PulseSourceRequestSchema, type PulseSourceResponse } from '@/lib/pulse/types'
import { getWorkspaceRoot } from '@/lib/pulse/workspace-root'

const SOURCE_INDEX_TIMEOUT_MS = 8 * 60_000

interface CommandResult {
  ok: boolean
  output: string
  /** Scraped markdown keyed by URL, parsed from --json stdout. */
  markdownBySrc: Record<string, string>
}

function runAxonScrape(urls: string[]): Promise<CommandResult> {
  return new Promise((resolve) => {
    const repoRoot = getWorkspaceRoot()
    const commandPath = path.join(repoRoot, 'scripts', 'axon')
    // --json: emit structured JSON per URL (includes markdown field)
    // --embed is true by default, so content is indexed into Qdrant
    const args = ['scrape', ...urls, '--json']
    const child = spawn(commandPath, args, {
      cwd: repoRoot,
      env: process.env,
      stdio: ['ignore', 'pipe', 'pipe'],
    })

    let stdout = ''
    let stderr = ''
    const timer = setTimeout(() => {
      child.kill('SIGTERM')
    }, SOURCE_INDEX_TIMEOUT_MS)

    child.stdout.on('data', (chunk: Buffer) => {
      stdout += chunk.toString()
    })
    child.stderr.on('data', (chunk: Buffer) => {
      stderr += chunk.toString()
    })

    child.on('error', (error: Error) => {
      clearTimeout(timer)
      resolve({
        ok: false,
        output: `Failed to start source indexing: ${error.message}`,
        markdownBySrc: {},
      })
    })

    child.on('close', (code: number | null, signal: NodeJS.Signals | null) => {
      clearTimeout(timer)
      if (signal) {
        resolve({
          ok: false,
          output: `Source indexing terminated by signal ${signal}`,
          markdownBySrc: {},
        })
        return
      }

      // Parse JSON lines from stdout to extract markdown per URL.
      const markdownBySrc: Record<string, string> = {}
      for (const line of stdout.split('\n')) {
        const trimmed = line.trim()
        if (!trimmed) continue
        try {
          const parsed = JSON.parse(trimmed) as Record<string, unknown>
          if (typeof parsed.url === 'string' && typeof parsed.markdown === 'string') {
            markdownBySrc[parsed.url] = parsed.markdown
          }
        } catch {
          // not a JSON line — options/progress text
        }
      }

      resolve({
        ok: code === 0,
        output: `${stdout}\n${stderr}`.trim(),
        markdownBySrc,
      })
    })
  })
}

export async function POST(request: Request) {
  ensureRepoRootEnvLoaded()

  let body: unknown
  try {
    body = await request.json()
  } catch {
    return NextResponse.json({ error: 'Request body must be valid JSON' }, { status: 400 })
  }

  const parsed = PulseSourceRequestSchema.safeParse(body)
  if (!parsed.success) {
    return NextResponse.json(
      { error: parsed.error.issues[0]?.message ?? 'Invalid request payload' },
      { status: 400 },
    )
  }

  const { urls } = parsed.data
  const result = await runAxonScrape(urls)
  if (!result.ok) {
    return NextResponse.json(
      { error: 'Source indexing failed', detail: result.output.slice(0, 6000) },
      { status: 502 },
    )
  }

  return NextResponse.json({
    indexed: urls,
    command: `./scripts/axon scrape ${urls.join(' ')} --json`,
    output: result.output.slice(0, 6000),
    markdownBySrc: result.markdownBySrc,
  } satisfies PulseSourceResponse)
}
