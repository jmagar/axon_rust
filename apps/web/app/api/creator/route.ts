import { promises as fs } from 'node:fs'
import path from 'node:path'
import { type NextRequest, NextResponse } from 'next/server'

// Claude CLI config dir inside the axon-web container (node user's home)
const CLAUDE_ROOT = path.resolve(process.env.CLAUDE_CONFIG ?? '/home/node/.claude')
const CLAUDE_PREFIX = '__claude'

// Categories scanned by the creator dashboard
export const CREATOR_CATEGORIES = [
  { name: 'skills', label: 'Skills' },
  { name: 'agents', label: 'Agents' },
  { name: 'commands', label: 'Commands' },
  { name: 'hooks', label: 'Hooks' },
] as const

export type CreatorCategory = (typeof CREATOR_CATEGORIES)[number]['name']

export interface CreatorFile {
  name: string
  path: string
  size: number
}

export interface CreatorCategoryResult {
  name: CreatorCategory
  label: string
  files: CreatorFile[]
}

/** Returns safe resolved path within CLAUDE_ROOT, or an error string */
function validateClaudePath(raw: string): { safe: string } | { error: string } {
  if (raw !== CLAUDE_PREFIX && !raw.startsWith(`${CLAUDE_PREFIX}/`)) {
    return { error: 'Only __claude/* paths are allowed' }
  }
  const relative = raw.length > CLAUDE_PREFIX.length ? raw.slice(CLAUDE_PREFIX.length + 1) : ''
  const resolved = relative ? path.resolve(CLAUDE_ROOT, relative) : CLAUDE_ROOT
  if (resolved !== CLAUDE_ROOT && !resolved.startsWith(CLAUDE_ROOT + path.sep)) {
    return { error: 'Path is outside Claude config' }
  }
  return { safe: resolved }
}

/** Re-validates symlink-resolved real path to prevent traversal */
async function realpathGuard(safePath: string): Promise<string> {
  try {
    const real = await fs.realpath(safePath)
    if (real !== CLAUDE_ROOT && !real.startsWith(CLAUDE_ROOT + path.sep)) {
      throw Object.assign(new Error('Path is outside Claude config'), { status: 400 })
    }
    return real
  } catch (err: unknown) {
    if (err instanceof Error && 'status' in err) throw err
    // ENOENT: path doesn't exist yet — let downstream stat/readFile throw naturally
    return safePath
  }
}

/** Scan a single category directory and return file entries */
async function scanCategory(
  categoryName: CreatorCategory,
  label: string,
): Promise<CreatorCategoryResult> {
  const dirPath = path.join(CLAUDE_ROOT, categoryName)
  const result: CreatorCategoryResult = { name: categoryName, label, files: [] }

  try {
    const entries = await fs.readdir(dirPath, { withFileTypes: true })
    const files = (
      await Promise.all(
        entries
          .filter((e) => !e.name.startsWith('.'))
          .map(async (e): Promise<CreatorFile | null> => {
            const absPath = path.join(dirPath, e.name)
            if (e.isFile()) {
              const filePath = `${CLAUDE_PREFIX}/${categoryName}/${e.name}`
              try {
                const stat = await fs.stat(absPath)
                return { name: e.name, path: filePath, size: stat.size }
              } catch {
                return { name: e.name, path: filePath, size: 0 }
              }
            }
            if (e.isDirectory()) {
              // Skill packages: look for SKILL.md, then README.md, then first .md file
              const candidates = ['SKILL.md', 'README.md']
              for (const candidate of candidates) {
                const candidatePath = path.join(absPath, candidate)
                try {
                  const stat = await fs.stat(candidatePath)
                  return {
                    name: e.name,
                    path: `${CLAUDE_PREFIX}/${categoryName}/${e.name}/${candidate}`,
                    size: stat.size,
                  }
                } catch {
                  // not found, try next
                }
              }
              // Fall back to first .md file in the directory
              try {
                const inner = await fs.readdir(absPath)
                const md = inner.find((f) => f.endsWith('.md'))
                if (md) {
                  const stat = await fs.stat(path.join(absPath, md))
                  return {
                    name: e.name,
                    path: `${CLAUDE_PREFIX}/${categoryName}/${e.name}/${md}`,
                    size: stat.size,
                  }
                }
              } catch {
                // ignore
              }
            }
            return null
          }),
      )
    ).filter((f): f is CreatorFile => f !== null)
    result.files = files.sort((a, b) => a.name.localeCompare(b.name))
  } catch {
    // Directory doesn't exist — return empty files array
  }

  return result
}

export async function GET(req: NextRequest) {
  const { searchParams } = req.nextUrl
  const action = searchParams.get('action') ?? 'list'
  const rawPath = searchParams.get('path') ?? ''

  if (action === 'list') {
    const categories = await Promise.all(
      CREATOR_CATEGORIES.map((cat) => scanCategory(cat.name, cat.label)),
    )
    return NextResponse.json({ categories })
  }

  if (action === 'read') {
    if (!rawPath) {
      return NextResponse.json({ error: 'path is required' }, { status: 400 })
    }

    const validation = validateClaudePath(rawPath)
    if ('error' in validation) {
      return NextResponse.json({ error: validation.error }, { status: 400 })
    }

    let safePath: string
    try {
      safePath = await realpathGuard(validation.safe)
    } catch {
      return NextResponse.json({ error: 'Path is outside allowed root' }, { status: 400 })
    }

    try {
      const stat = await fs.stat(safePath)
      if (stat.isDirectory()) {
        return NextResponse.json({ error: 'Is a directory' }, { status: 400 })
      }
      if (stat.size > 1_000_000) {
        return NextResponse.json({ error: 'File too large (>1MB)' }, { status: 413 })
      }
      const content = await fs.readFile(safePath, 'utf8')
      return NextResponse.json({
        name: path.basename(safePath),
        size: stat.size,
        modified: stat.mtime.toISOString(),
        content,
      })
    } catch {
      return NextResponse.json({ error: 'File not found' }, { status: 404 })
    }
  }

  return NextResponse.json({ error: 'Unknown action' }, { status: 400 })
}

export async function POST(req: NextRequest) {
  let body: unknown
  try {
    body = await req.json()
  } catch {
    return NextResponse.json({ error: 'Invalid JSON body' }, { status: 400 })
  }

  if (
    typeof body !== 'object' ||
    body === null ||
    typeof (body as Record<string, unknown>).path !== 'string' ||
    typeof (body as Record<string, unknown>).content !== 'string'
  ) {
    return NextResponse.json({ error: 'Missing path or content' }, { status: 400 })
  }

  const { path: rawPath, content } = body as { path: string; content: string }

  const validation = validateClaudePath(rawPath)
  if ('error' in validation) {
    return NextResponse.json({ error: validation.error }, { status: 400 })
  }

  let safePath: string
  try {
    safePath = await realpathGuard(validation.safe)
  } catch {
    return NextResponse.json({ error: 'Path is outside allowed root' }, { status: 400 })
  }

  try {
    await fs.mkdir(path.dirname(safePath), { recursive: true })
    await fs.writeFile(safePath, content, 'utf8')
    return NextResponse.json({ ok: true })
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Write failed'
    return NextResponse.json({ error: msg }, { status: 500 })
  }
}
