import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { NextResponse } from 'next/server'
import { z } from 'zod'

const McpServerConfigSchema = z.object({
  command: z
    .string()
    // Allow only safe path characters and reject path-traversal sequences (..)
    .regex(/^(?!.*\.\.)([/a-zA-Z0-9._-]+)$/)
    .optional(),
  args: z.array(z.string().max(500)).max(20).optional(),
  env: z.record(z.string().regex(/^[A-Z_][A-Z0-9_]*/), z.string().max(1000)).optional(),
  url: z.string().url().optional(),
  headers: z.record(z.string().max(200), z.string().max(1000)).optional(),
})

const McpConfigSchema = z.object({
  mcpServers: z
    .record(z.string().max(100), McpServerConfigSchema)
    .refine((obj) => Object.keys(obj).length <= 50, { message: 'Too many servers (max 50)' }),
})

type McpConfig = z.infer<typeof McpConfigSchema>

const MCP_JSON_PATH = path.join(os.homedir(), '.claude', 'mcp.json')

async function readMcpConfig(): Promise<McpConfig> {
  try {
    const raw = await fs.readFile(MCP_JSON_PATH, 'utf8')
    const parsed = JSON.parse(raw) as McpConfig
    if (!parsed.mcpServers || typeof parsed.mcpServers !== 'object') {
      return { mcpServers: {} }
    }
    return parsed
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      return { mcpServers: {} }
    }
    throw err
  }
}

async function writeMcpConfig(config: McpConfig): Promise<void> {
  const dir = path.dirname(MCP_JSON_PATH)
  await fs.mkdir(dir, { recursive: true })
  await fs.writeFile(MCP_JSON_PATH, JSON.stringify(config, null, 2), 'utf8')
}

export async function GET() {
  try {
    const config = await readMcpConfig()
    return NextResponse.json(config)
  } catch (err) {
    console.error('[MCP] GET failed:', err)
    return NextResponse.json({ error: 'Failed to read mcp.json' }, { status: 500 })
  }
}

export async function PUT(request: Request) {
  // Requires X-Pulse-Request: 1 header — added by mcp/page.tsx fetch calls
  if (request.headers.get('X-Pulse-Request') !== '1') {
    return NextResponse.json({ error: 'Forbidden' }, { status: 403 })
  }
  try {
    const body = (await request.json()) as unknown
    const result = McpConfigSchema.safeParse(body)
    if (!result.success) {
      return NextResponse.json(
        {
          error: 'Body must have mcpServers: Record<string, McpServerConfig>',
          details: result.error.flatten(),
        },
        { status: 400 },
      )
    }
    await writeMcpConfig(result.data)
    return NextResponse.json({ ok: true })
  } catch (err) {
    console.error('[MCP] PUT failed:', err)
    return NextResponse.json({ error: 'Failed to write mcp.json' }, { status: 500 })
  }
}

export async function DELETE(request: Request) {
  // Requires X-Pulse-Request: 1 header — added by mcp/page.tsx fetch calls
  if (request.headers.get('X-Pulse-Request') !== '1') {
    return NextResponse.json({ error: 'Forbidden' }, { status: 403 })
  }
  try {
    const body = (await request.json()) as unknown
    if (
      !body ||
      typeof body !== 'object' ||
      !('name' in body) ||
      typeof (body as { name: unknown }).name !== 'string'
    ) {
      return NextResponse.json({ error: 'Body must have name: string' }, { status: 400 })
    }
    const { name } = body as { name: string }
    const config = await readMcpConfig()
    const updated: McpConfig = {
      mcpServers: Object.fromEntries(Object.entries(config.mcpServers).filter(([k]) => k !== name)),
    }
    await writeMcpConfig(updated)
    return NextResponse.json({ ok: true })
  } catch (err) {
    console.error('[MCP] DELETE failed:', err)
    return NextResponse.json({ error: 'Failed to delete server from mcp.json' }, { status: 500 })
  }
}
