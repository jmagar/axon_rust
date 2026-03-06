import crypto from 'node:crypto'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'

export interface SessionFile {
  id: string
  absolutePath: string
  project: string
  filename: string
  mtimeMs: number
  sizeBytes: number
  preview?: string
}

function selectPreferredSession(current: SessionFile, next: SessionFile): SessionFile {
  if (next.mtimeMs !== current.mtimeMs) {
    return next.mtimeMs > current.mtimeMs ? next : current
  }
  if (current.project === 'tmp' && next.project !== 'tmp') return next
  if (next.project === 'tmp' && current.project !== 'tmp') return current
  if (next.sizeBytes !== current.sizeBytes) {
    return next.sizeBytes > current.sizeBytes ? next : current
  }
  return current
}

/** Patterns that indicate a message is a system/handoff prompt, not real user input. */
const SKIP_PATTERNS = [/^Respond as JSON/, /^I'm loading a previous/, /^## Context/]
const PREVIEW_TRUNCATE_PATTERNS = [
  /\s+Respond as JSON only with this exact shape:.*/i,
  /\s+Respond as JSON only with this shape:.*/i,
]

const normalizePreviewText = (text: string): string => {
  let out = text.trim().replace(/\n+/g, ' ')
  for (const pattern of PREVIEW_TRUNCATE_PATTERNS) {
    out = out.replace(pattern, '').trim()
  }
  return out
}

/**
 * Read up to the first 4KB of a JSONL session file and extract the first
 * meaningful user message as a short preview string (≤80 chars).
 * Never throws — returns undefined on any error or if no good message is found.
 */
async function extractPreview(absolutePath: string): Promise<string | undefined> {
  try {
    const fd = await fs.open(absolutePath, 'r')
    try {
      const buf = Buffer.allocUnsafe(4096)
      const { bytesRead } = await fd.read(buf, 0, 4096, 0)
      const chunk = buf.subarray(0, bytesRead).toString('utf8')

      // Work line-by-line; take at most the first 20 lines to stay fast.
      const lines = chunk.split('\n').slice(0, 20)

      for (const line of lines) {
        const trimmed = line.trim()
        if (!trimmed) continue

        let val: Record<string, unknown>
        try {
          val = JSON.parse(trimmed) as Record<string, unknown>
        } catch {
          continue
        }

        if (val.type !== 'user') continue

        const msg = val.message as Record<string, unknown> | undefined
        const msgContent = msg?.content

        let text = ''
        if (typeof msgContent === 'string') {
          text = msgContent
        } else if (Array.isArray(msgContent)) {
          for (const block of msgContent) {
            const blockText = (block as Record<string, unknown>).text
            if (typeof blockText === 'string') text += `${blockText}\n`
          }
        }

        text = normalizePreviewText(text)
        if (!text) continue

        // Skip system-like / handoff messages.
        if (SKIP_PATTERNS.some((re) => re.test(text))) continue

        // Skip very long unstructured blobs (likely injected context, not real questions).
        if (text.length > 500 && !/[.?!]/.test(text.slice(0, 200))) continue

        // We have a good candidate — trim to 80 chars.
        return text.length > 80 ? `${text.slice(0, 80)}…` : text
      }

      return undefined
    } finally {
      await fd.close()
    }
  } catch {
    return undefined
  }
}

/**
 * Port of clean_claude_project_name from crates/ingest/sessions/claude.rs.
 * Converts a directory name like "-home-jmagar-workspace-axon-rust" to
 * a human-readable project name like "axon-rust".
 */
// Words that indicate a suffix rather than the project name itself.
const SUFFIX_WORDS = new Set(['rust', 'rs', 'git', 'main', 'master', 'src'])

export function cleanProjectName(dirName: string): string {
  if (!dirName.includes('-')) return dirName
  const parts = dirName.replace(/^-+/, '').split('-').filter(Boolean)
  if (parts.length === 0) return dirName
  if (parts.length === 1) return parts[0] ?? dirName

  const last = parts[parts.length - 1] ?? ''
  const prev = parts[parts.length - 2] ?? ''

  // If the last segment is a known suffix, drop it and return just prev.
  // Otherwise show the last two path segments for context (e.g., "my-project").
  return SUFFIX_WORDS.has(last) ? prev : `${prev}-${last}`
}

function sessionId(absolutePath: string): string {
  return crypto.createHash('sha256').update(absolutePath).digest('hex').slice(0, 12)
}

async function readDirEntries(dirPath: string): Promise<string[]> {
  try {
    return await fs.readdir(dirPath)
  } catch {
    return []
  }
}

async function isDirEntry(entryPath: string): Promise<boolean> {
  try {
    const stat = await fs.stat(entryPath)
    return stat.isDirectory()
  } catch {
    return false
  }
}

/**
 * Scan ~/.claude/projects/**\/*.jsonl, return metadata sorted by mtime desc.
 * Never throws — returns [] on any filesystem error.
 */
export async function scanSessions(limit = 20): Promise<SessionFile[]> {
  const root = path.join(os.homedir(), '.claude', 'projects')

  try {
    await fs.access(root)
  } catch {
    return []
  }

  const results: SessionFile[] = []
  const projectNames = await readDirEntries(root)

  for (const projectName of projectNames) {
    const projectPath = path.join(root, projectName)
    if (!projectPath.startsWith(root + path.sep)) continue
    if (!(await isDirEntry(projectPath))) continue

    const fileNames = await readDirEntries(projectPath)
    for (const fileName of fileNames) {
      if (!fileName.endsWith('.jsonl')) continue
      const absolutePath = path.join(projectPath, fileName)
      if (!absolutePath.startsWith(root + path.sep)) continue
      try {
        const stat = await fs.stat(absolutePath)
        if (!stat.isFile()) continue
        results.push({
          id: sessionId(absolutePath),
          absolutePath,
          project: cleanProjectName(projectName),
          filename: fileName.slice(0, -'.jsonl'.length),
          mtimeMs: stat.mtimeMs,
          sizeBytes: stat.size,
          preview: await extractPreview(absolutePath),
        })
      } catch {
        // skip unreadable files
      }
    }
  }

  const deduped = new Map<string, SessionFile>()
  for (const session of results) {
    const key = session.absolutePath
    const existing = deduped.get(key)
    deduped.set(key, existing ? selectPreferredSession(existing, session) : session)
  }

  return Array.from(deduped.values())
    .sort((a, b) => b.mtimeMs - a.mtimeMs)
    .slice(0, limit)
}
