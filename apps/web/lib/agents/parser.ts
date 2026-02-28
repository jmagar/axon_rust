export interface Agent {
  name: string
  description: string
  source: string
}

/**
 * Parses the stdout of `claude agents` into a structured list of agents and groups.
 *
 * Parser rules:
 *  - Group header: no leading whitespace, ends with ':'
 *  - Agent line: starts with 2 spaces, contains ' — ' (em-dash with spaces)
 *  - Description: everything after the FIRST occurrence of ' — '
 */
export function parseAgentsOutput(stdout: string): { agents: Agent[]; groups: string[] } {
  const agents: Agent[] = []
  const groups: string[] = []
  let currentGroup = ''

  for (const raw of stdout.split('\n')) {
    const line = raw.trimEnd()
    if (!line) continue

    // Group header: no leading whitespace, ends with ':'
    if (!line.startsWith(' ') && line.endsWith(':')) {
      currentGroup = line.slice(0, -1).trim()
      if (!groups.includes(currentGroup)) {
        groups.push(currentGroup)
      }
      continue
    }

    // Agent line: starts with 2 spaces and contains ' — '
    if (line.startsWith('  ') && line.includes(' \u2014 ')) {
      const trimmed = line.trim()
      const sepIdx = trimmed.indexOf(' \u2014 ')
      if (sepIdx !== -1) {
        const name = trimmed.slice(0, sepIdx).trim()
        const description = trimmed.slice(sepIdx + 3).trim()
        agents.push({ name, description, source: currentGroup })
      }
    }
  }

  return { agents, groups }
}
