import { execFile } from 'node:child_process'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { NextResponse } from 'next/server'

const execFileAsync = promisify(execFile)

type McpServerConfig = {
  command?: string
  args?: string[]
  env?: Record<string, string>
  url?: string
  headers?: Record<string, string>
}

type McpConfig = {
  mcpServers: Record<string, McpServerConfig>
}

type ServerStatus = 'online' | 'offline' | 'unknown'

const MCP_JSON_PATH = path.join(os.homedir(), '.claude', 'mcp.json')

const BLOCKED_HOSTNAMES = new Set(['localhost', '127.0.0.1', '0.0.0.0', '::1'])
const PRIVATE_IP_PATTERNS = [
  /^127\./,
  /^10\./,
  /^172\.(1[6-9]|2[0-9]|3[01])\./,
  /^192\.168\./,
  /^169\.254\./,
  /^fc[0-9a-f]{2}:/i,
  /^fd[0-9a-f]{2}:/i,
]

function validateStatusUrl(url: string): boolean {
  let parsed: URL
  try {
    parsed = new URL(url)
  } catch {
    return false
  }
  if (!['http:', 'https:'].includes(parsed.protocol)) return false
  // parsed.hostname includes brackets for IPv6 addresses (e.g. "[::1]") — strip them.
  const hostname = parsed.hostname.replace(/^\[|\]$/g, '')
  if (BLOCKED_HOSTNAMES.has(hostname)) return false
  if (PRIVATE_IP_PATTERNS.some((p) => p.test(hostname))) return false
  return true
}

async function readMcpConfig(): Promise<McpConfig> {
  try {
    const raw = await fs.readFile(MCP_JSON_PATH, 'utf8')
    const parsed = JSON.parse(raw) as McpConfig
    if (!parsed.mcpServers || typeof parsed.mcpServers !== 'object') {
      return { mcpServers: {} }
    }
    return parsed
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') return { mcpServers: {} }
    throw err
  }
}

async function checkHttpServer(url: string): Promise<ServerStatus> {
  try {
    const signal = AbortSignal.timeout(4_000)
    const res = await fetch(url, { method: 'HEAD', signal })
    // Any HTTP response (even 404/405) means the server is reachable
    return res.status < 600 ? 'online' : 'offline'
  } catch {
    return 'offline'
  }
}

async function checkStdioServer(command: string): Promise<ServerStatus> {
  if (!command.trim()) return 'unknown'
  // Reject commands containing path separators to prevent directory traversal
  if (command.includes('/') || command.includes('\\')) return 'offline'
  try {
    // Absolute path → check file existence directly
    if (path.isAbsolute(command)) {
      await fs.access(command)
      return 'online'
    }
    // Relative command → check if it's on PATH
    await execFileAsync('which', [command], { timeout: 3_000 })
    return 'online'
  } catch {
    return 'offline'
  }
}

export async function GET() {
  try {
    const config = await readMcpConfig()
    const entries = Object.entries(config.mcpServers)

    const checks = entries.map(
      async ([name, cfg]): Promise<[string, { status: ServerStatus; error?: string }]> => {
        if (cfg.url) {
          if (!validateStatusUrl(cfg.url)) {
            return [name, { status: 'offline', error: 'invalid_url' }]
          }
          const status = await checkHttpServer(cfg.url)
          return [name, { status }]
        }
        if (cfg.command) {
          const status = await checkStdioServer(cfg.command)
          return [name, { status }]
        }
        return [name, { status: 'unknown' }]
      },
    )

    const results = await Promise.all(checks)
    const servers = Object.fromEntries(results)

    return NextResponse.json({ servers })
  } catch (err) {
    console.error('[MCP status] GET failed:', err)
    return NextResponse.json({ error: 'Failed to check MCP server status' }, { status: 500 })
  }
}
