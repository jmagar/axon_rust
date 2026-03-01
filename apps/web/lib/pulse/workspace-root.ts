import path from 'node:path'

/** Resolve repository root from any cwd under apps/web or at repository root. */
export function getWorkspaceRoot(cwd = process.cwd()): string {
  const normalized = path.normalize(cwd)
  const marker = `${path.sep}apps${path.sep}web`
  const markerIndex = normalized.lastIndexOf(marker)
  if (markerIndex === -1) return normalized
  return normalized.slice(0, markerIndex)
}
