import fs from 'node:fs'
import path from 'node:path'
import { getWorkspaceRoot } from './workspace-root'

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

  const repoRoot = getWorkspaceRoot()
  const envPath = path.join(repoRoot, '.env')

  if (!fs.existsSync(envPath)) {
    rootEnvLoaded = true
    return
  }

  try {
    const lines = fs.readFileSync(envPath, 'utf8').split(/\r?\n/)
    for (const line of lines) {
      const parsed = parseDotenvLine(line)
      if (!parsed) continue
      const [key, value] = parsed
      if (process.env[key] === undefined) {
        process.env[key] = value
      }
    }
    rootEnvLoaded = true
  } catch {
    // Keep request path resilient if repo root .env exists but is unreadable.
    // Retry on the next request in case this was a transient filesystem error.
  }
}
