import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'

const PULSE_DIR = path.resolve(process.cwd(), '.cache/pulse')

interface SavePayload {
  title: string
  markdown: string
  tags?: string[]
  collections?: string[]
}

interface StoredDoc {
  title: string
  markdown: string
  tags: string[]
  collections: string[]
  createdAt: string
  updatedAt: string
}

function slugify(input: string): string {
  const cleaned = input
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .trim()
    .replace(/\s+/g, '-')
  return cleaned || 'untitled'
}

function toFrontmatter(doc: StoredDoc): string {
  const tags = JSON.stringify(doc.tags)
  const collections = JSON.stringify(doc.collections)
  return [
    '---',
    `title: ${JSON.stringify(doc.title)}`,
    `createdAt: ${JSON.stringify(doc.createdAt)}`,
    `updatedAt: ${JSON.stringify(doc.updatedAt)}`,
    `tags: ${tags}`,
    `collections: ${collections}`,
    '---',
    '',
    doc.markdown,
  ].join('\n')
}

function parseFrontmatter(raw: string): StoredDoc | null {
  if (!raw.startsWith('---\n')) return null
  const end = raw.indexOf('\n---\n', 4)
  if (end < 0) return null
  const metaRaw = raw.slice(4, end)
  const markdown = raw.slice(end + 5)
  const meta = Object.fromEntries(
    metaRaw
      .split('\n')
      .map((line) => line.split(':').map((part) => part.trim()))
      .filter((parts) => parts.length >= 2)
      .map(([key, ...rest]) => [key, rest.join(':').trim()]),
  )

  const parseMaybeJson = (value: string | undefined): unknown => {
    if (!value) return undefined
    try {
      return JSON.parse(value)
    } catch {
      return value
    }
  }

  return {
    title: String(parseMaybeJson(meta.title) ?? 'Untitled'),
    markdown,
    tags: (parseMaybeJson(meta.tags) as string[] | undefined) ?? [],
    collections: (parseMaybeJson(meta.collections) as string[] | undefined) ?? ['pulse'],
    createdAt: String(parseMaybeJson(meta.createdAt) ?? new Date().toISOString()),
    updatedAt: String(parseMaybeJson(meta.updatedAt) ?? new Date().toISOString()),
  }
}

export async function savePulseDoc(
  payload: SavePayload,
): Promise<{ path: string; filename: string }> {
  await mkdir(PULSE_DIR, { recursive: true })
  const timestamp = Date.now()
  const filename = `${slugify(payload.title)}-${timestamp}.md`
  const filePath = path.join(PULSE_DIR, filename)
  const now = new Date().toISOString()
  const doc: StoredDoc = {
    title: payload.title,
    markdown: payload.markdown,
    tags: payload.tags ?? [],
    collections: payload.collections ?? ['pulse'],
    createdAt: now,
    updatedAt: now,
  }

  await writeFile(filePath, toFrontmatter(doc), 'utf-8')
  return { path: filePath, filename }
}

export async function loadPulseDoc(filename: string): Promise<StoredDoc | null> {
  const safeName = path.basename(filename)
  const fullPath = path.join(PULSE_DIR, safeName)
  try {
    const raw = await readFile(fullPath, 'utf-8')
    return parseFrontmatter(raw)
  } catch {
    return null
  }
}

export async function listPulseDocs(): Promise<
  Array<{ filename: string; title: string; updatedAt: string }>
> {
  await mkdir(PULSE_DIR, { recursive: true })
  const entries = await readdir(PULSE_DIR)
  const docs = await Promise.all(
    entries
      .filter((name) => name.endsWith('.md'))
      .map(async (filename) => {
        const doc = await loadPulseDoc(filename)
        if (!doc) return null
        return { filename, title: doc.title, updatedAt: doc.updatedAt }
      }),
  )

  return docs
    .filter((doc): doc is { filename: string; title: string; updatedAt: string } => doc !== null)
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
}
