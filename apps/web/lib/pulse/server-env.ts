import fs from 'node:fs'
import path from 'node:path'

let rootEnvLoaded = false

function parseDotenvLine(line: string): [string, string] | null {
  const trimmed = line.trim()
  if (!trimmed || trimmed.startsWith('#')) return null

  const eq = trimmed.indexOf('=')
  if (eq <= 0) return null

  const key = trimmed.slice(0, eq).trim()
  let value = trimmed.slice(eq + 1).trim()

  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    value = value.slice(1, -1)
  }

  return [key, value]
}

export function ensureRepoRootEnvLoaded() {
  if (rootEnvLoaded) return
  rootEnvLoaded = true

  // apps/web -> repo root
  const repoRoot = path.resolve(process.cwd(), '..', '..')
  const envPath = path.join(repoRoot, '.env')

  if (!fs.existsSync(envPath)) return

  const lines = fs.readFileSync(envPath, 'utf8').split(/\r?\n/)
  for (const line of lines) {
    const parsed = parseDotenvLine(line)
    if (!parsed) continue
    const [key, value] = parsed
    if (process.env[key] === undefined) {
      process.env[key] = value
    }
  }
}
