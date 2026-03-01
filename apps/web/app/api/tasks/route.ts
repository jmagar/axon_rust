import { spawn } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { promises as fs } from 'node:fs'
import path from 'node:path'
import { type NextRequest, NextResponse } from 'next/server'

// ── Constants ──────────────────────────────────────────────────────────────────

const WORKSPACE_ROOT = process.env.AXON_WORKSPACE ?? '/workspace'
const TASKS_FILE = path.join(WORKSPACE_ROOT, '.cache', 'axon-tasks.json')
const RUNS_FILE = path.join(WORKSPACE_ROOT, '.cache', 'axon-task-runs.json')

// ── Types ──────────────────────────────────────────────────────────────────────

export interface Task {
  id: string
  name: string
  description?: string
  schedule: string
  command: string
  enabled: boolean
  createdAt: string
  updatedAt: string
}

export interface TaskRun {
  id: string
  taskId: string
  startedAt: string
  finishedAt?: string
  status: 'running' | 'completed' | 'failed'
  output?: string
  error?: string
}

// ── Validation ─────────────────────────────────────────────────────────────────

const SHELL_META = /[;|&><`$]/
const CRON_RE = /^(\S+\s+){4}\S+(\s+\S+){0,2}$/

function validateTask(body: Partial<Task>): string | null {
  if (!body.name || typeof body.name !== 'string') return 'name is required'
  if (body.name.length > 100) return 'name must be 100 characters or fewer'
  if (!body.command || typeof body.command !== 'string') return 'command is required'
  if (SHELL_META.test(body.command))
    return 'command must not contain shell metacharacters (; | & > < ` $)'
  if (!body.schedule || typeof body.schedule !== 'string') return 'schedule is required'
  if (body.schedule !== 'once' && !CRON_RE.test(body.schedule.trim())) {
    return 'schedule must be "once" or a valid cron expression (5–7 space-separated tokens)'
  }
  return null
}

// ── File helpers ───────────────────────────────────────────────────────────────

async function ensureCacheDir(): Promise<void> {
  await fs.mkdir(path.join(WORKSPACE_ROOT, '.cache'), { recursive: true })
}

async function readTasks(): Promise<Task[]> {
  try {
    const raw = await fs.readFile(TASKS_FILE, 'utf8')
    return JSON.parse(raw) as Task[]
  } catch {
    return []
  }
}

async function writeTasks(tasks: Task[]): Promise<void> {
  await ensureCacheDir()
  await fs.writeFile(TASKS_FILE, JSON.stringify(tasks, null, 2), 'utf8')
}

async function readRuns(): Promise<TaskRun[]> {
  try {
    const raw = await fs.readFile(RUNS_FILE, 'utf8')
    return JSON.parse(raw) as TaskRun[]
  } catch {
    return []
  }
}

async function writeRuns(runs: TaskRun[]): Promise<void> {
  await ensureCacheDir()
  await fs.writeFile(RUNS_FILE, JSON.stringify(runs, null, 2), 'utf8')
}

// ── Manual run ────────────────────────────────────────────────────────────────

function spawnCommand(command: string): Promise<{ output: string; error?: string }> {
  return new Promise((resolve) => {
    const parts = command.trim().split(/\s+/)
    const bin = parts[0] ?? ''
    const args = parts.slice(1)
    const proc = spawn(bin, args, { shell: false, timeout: 300_000 })

    let stdout = ''
    let stderr = ''
    proc.stdout?.on('data', (chunk: Buffer) => {
      stdout += chunk.toString()
    })
    proc.stderr?.on('data', (chunk: Buffer) => {
      stderr += chunk.toString()
    })

    proc.on('close', (code) => {
      if (code === 0) {
        resolve({ output: stdout })
      } else {
        resolve({ output: stdout, error: stderr || `Process exited with code ${code}` })
      }
    })

    proc.on('error', (err) => {
      resolve({ output: '', error: err.message })
    })
  })
}

// ── Route handlers ─────────────────────────────────────────────────────────────

export async function GET(req: NextRequest) {
  const { searchParams } = req.nextUrl
  const id = searchParams.get('id')

  const tasks = await readTasks()

  if (id) {
    const task = tasks.find((t) => t.id === id)
    if (!task) return NextResponse.json({ error: 'Task not found' }, { status: 404 })
    const runs = await readRuns()
    const recentRuns = runs
      .filter((r) => r.taskId === id)
      .sort((a, b) => b.startedAt.localeCompare(a.startedAt))
      .slice(0, 20)
    return NextResponse.json({ task, runs: recentRuns })
  }

  return NextResponse.json({ tasks })
}

export async function POST(req: NextRequest) {
  // Manual run trigger
  if (req.nextUrl.pathname.endsWith('/run')) {
    const { searchParams } = req.nextUrl
    const id = searchParams.get('id')
    if (!id) return NextResponse.json({ error: 'id is required' }, { status: 400 })

    const tasks = await readTasks()
    const task = tasks.find((t) => t.id === id)
    if (!task) return NextResponse.json({ error: 'Task not found' }, { status: 404 })

    const runId = randomUUID()
    const now = new Date().toISOString()
    const runs = await readRuns()
    const run: TaskRun = { id: runId, taskId: id, startedAt: now, status: 'running' }
    runs.push(run)
    await writeRuns(runs)

    // Run command asynchronously — persist result when done
    void spawnCommand(task.command).then(async ({ output, error }) => {
      const allRuns = await readRuns()
      const idx = allRuns.findIndex((r) => r.id === runId)
      if (idx !== -1) {
        allRuns[idx] = {
          ...allRuns[idx],
          finishedAt: new Date().toISOString(),
          status: error ? 'failed' : 'completed',
          output: output || undefined,
          error: error || undefined,
        } as TaskRun
        await writeRuns(allRuns)
      }
    })

    return NextResponse.json({ runId })
  }

  // Create task
  let body: Partial<Task>
  try {
    body = (await req.json()) as Partial<Task>
  } catch {
    return NextResponse.json({ error: 'Invalid JSON body' }, { status: 400 })
  }

  const validationError = validateTask(body)
  if (validationError) return NextResponse.json({ error: validationError }, { status: 400 })

  const now = new Date().toISOString()
  const task: Task = {
    id: randomUUID(),
    name: body.name!.trim(),
    description: body.description?.trim() || undefined,
    schedule: body.schedule!.trim(),
    command: body.command!.trim(),
    enabled: body.enabled ?? true,
    createdAt: now,
    updatedAt: now,
  }

  const tasks = await readTasks()
  tasks.push(task)
  await writeTasks(tasks)

  return NextResponse.json({ task }, { status: 201 })
}

export async function PUT(req: NextRequest) {
  let body: Partial<Task>
  try {
    body = (await req.json()) as Partial<Task>
  } catch {
    return NextResponse.json({ error: 'Invalid JSON body' }, { status: 400 })
  }

  if (!body.id) return NextResponse.json({ error: 'id is required' }, { status: 400 })

  const validationError = validateTask(body)
  if (validationError) return NextResponse.json({ error: validationError }, { status: 400 })

  const tasks = await readTasks()
  const idx = tasks.findIndex((t) => t.id === body.id)
  if (idx === -1) return NextResponse.json({ error: 'Task not found' }, { status: 404 })

  const updated: Task = {
    ...tasks[idx]!,
    name: body.name!.trim(),
    description: body.description?.trim() || undefined,
    schedule: body.schedule!.trim(),
    command: body.command!.trim(),
    enabled: body.enabled ?? tasks[idx]!.enabled,
    updatedAt: new Date().toISOString(),
  }
  tasks[idx] = updated
  await writeTasks(tasks)

  return NextResponse.json({ task: updated })
}

export async function DELETE(req: NextRequest) {
  const { searchParams } = req.nextUrl
  const id = searchParams.get('id')
  if (!id) return NextResponse.json({ error: 'id is required' }, { status: 400 })

  const tasks = await readTasks()
  const idx = tasks.findIndex((t) => t.id === id)
  if (idx === -1) return NextResponse.json({ error: 'Task not found' }, { status: 404 })

  tasks.splice(idx, 1)
  await writeTasks(tasks)

  return NextResponse.json({ ok: true })
}
