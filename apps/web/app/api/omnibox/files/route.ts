import { promises as fs } from 'node:fs'
import path from 'node:path'
import { NextResponse } from 'next/server'
import { listPulseDocs } from '@/lib/pulse/storage'

type LocalDocSource = 'docs' | 'pulse'

interface LocalDocFile {
  id: string
  label: string
  path: string
  source: LocalDocSource
  updatedAt?: string
}

const ALLOWED_DOC_EXTENSIONS = new Set(['.md', '.mdx', '.txt', '.rst'])

function getWorkspaceRoot(): string {
  // apps/web -> workspace root
  return path.resolve(process.cwd(), '..', '..')
}

function getRootBySource(workspaceRoot: string, source: LocalDocSource): string {
  if (source === 'docs') return path.join(workspaceRoot, 'docs')
  return path.join(workspaceRoot, '.cache', 'pulse')
}

async function collectDocsDir(rootDir: string): Promise<LocalDocFile[]> {
  async function walk(current: string, out: LocalDocFile[]) {
    const entries = await fs.readdir(current, { withFileTypes: true })
    for (const entry of entries) {
      const absolute = path.join(current, entry.name)
      if (entry.isDirectory()) {
        await walk(absolute, out)
        continue
      }
      if (!entry.isFile()) continue
      const ext = path.extname(entry.name).toLowerCase()
      if (!ALLOWED_DOC_EXTENSIONS.has(ext)) continue

      const relativePath = path.relative(rootDir, absolute)
      out.push({
        id: `docs:${relativePath}`,
        label: path.basename(relativePath, ext),
        path: path.join('docs', relativePath),
        source: 'docs',
      })
    }
  }

  const files: LocalDocFile[] = []
  try {
    await walk(rootDir, files)
  } catch {
    return []
  }
  return files
}

async function collectPulseDocs(): Promise<LocalDocFile[]> {
  try {
    const docs = await listPulseDocs()
    return docs.map((doc) => ({
      id: `pulse:${doc.filename}`,
      label: doc.title,
      path: path.join('.cache', 'pulse', doc.filename),
      source: 'pulse',
      updatedAt: doc.updatedAt,
    }))
  } catch {
    return []
  }
}

async function resolveFileById(workspaceRoot: string, id: string) {
  const splitIndex = id.indexOf(':')
  if (splitIndex <= 0) return null

  const source = id.slice(0, splitIndex) as LocalDocSource
  const relativePath = id.slice(splitIndex + 1)
  if (source !== 'docs' && source !== 'pulse') return null
  if (!relativePath || relativePath.includes('..')) return null

  const sourceRoot = getRootBySource(workspaceRoot, source)
  const absolutePath = path.resolve(sourceRoot, relativePath)
  if (!absolutePath.startsWith(sourceRoot)) return null

  try {
    const content = await fs.readFile(absolutePath, 'utf8')
    const ext = path.extname(relativePath)
    const label = path.basename(relativePath, ext)
    return {
      id,
      label,
      path:
        source === 'docs'
          ? path.join('docs', relativePath)
          : path.join('.cache', 'pulse', relativePath),
      source,
      content,
    }
  } catch {
    return null
  }
}

export async function GET(request: Request) {
  const url = new URL(request.url)
  const requestedId = url.searchParams.get('id')
  const workspaceRoot = getWorkspaceRoot()

  if (requestedId) {
    const file = await resolveFileById(workspaceRoot, requestedId)
    if (!file) return NextResponse.json({ error: 'Not found' }, { status: 404 })
    return NextResponse.json({ file })
  }

  const [docsFiles, pulseFiles] = await Promise.all([
    collectDocsDir(path.join(workspaceRoot, 'docs')),
    collectPulseDocs(),
  ])

  return NextResponse.json({ files: [...pulseFiles, ...docsFiles] })
}
