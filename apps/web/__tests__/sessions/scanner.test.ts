import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { cleanProjectName, scanSessions } from '@/lib/sessions/session-scanner'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a minimal valid JSONL line with a user message. */
function makeUserLine(content: string): string {
  return JSON.stringify({
    type: 'user',
    message: { content },
  })
}

// ---------------------------------------------------------------------------
// scanSessions
// ---------------------------------------------------------------------------

describe('scanSessions', () => {
  let tmpRoot: string
  let origHome: string

  beforeEach(async () => {
    // Create a temp directory that looks like ~/.claude/projects
    tmpRoot = await fs.mkdtemp(path.join(os.tmpdir(), 'axon-scanner-test-'))

    // Patch os.homedir so scanSessions resolves inside our temp dir.
    // scanSessions uses os.homedir() at call-time, so we must replace the
    // live binding.  The cleanest approach without monkeypatching the module
    // is to create the full directory hierarchy inside tmpdir and point
    // HOME at it — scanSessions joins homedir + '.claude/projects'.
    origHome = process.env.HOME ?? ''
    process.env.HOME = tmpRoot

    // Create the projects directory so the scanner doesn't bail early.
    await fs.mkdir(path.join(tmpRoot, '.claude', 'projects'), { recursive: true })
  })

  afterEach(async () => {
    process.env.HOME = origHome
    await fs.rm(tmpRoot, { recursive: true, force: true })
  })

  it('returns [] for empty directory', async () => {
    // projects dir exists but has no sub-directories
    const sessions = await scanSessions()
    expect(sessions).toEqual([])
  })

  it('returns session summary for valid .jsonl file', async () => {
    const projectsDir = path.join(tmpRoot, '.claude', 'projects')
    const projectDir = path.join(projectsDir, '-home-user-workspace-my-app')
    await fs.mkdir(projectDir, { recursive: true })

    const sessionFile = path.join(projectDir, 'abc123.jsonl')
    const jsonlContent = makeUserLine('What is the meaning of life?')
    await fs.writeFile(sessionFile, `${jsonlContent}\n`, 'utf8')

    const sessions = await scanSessions()

    expect(sessions).toHaveLength(1)
    const s = sessions[0]!
    expect(s.project).toBe('my-app')
    expect(s.filename).toBe('abc123')
    expect(s.absolutePath).toBe(sessionFile)
    expect(typeof s.id).toBe('string')
    expect(s.id).toHaveLength(12)
    expect(s.sizeBytes).toBeGreaterThan(0)
  })

  it('skips path traversal filenames like ../../evil.jsonl', async () => {
    const projectsDir = path.join(tmpRoot, '.claude', 'projects')
    const projectDir = path.join(projectsDir, '-home-user-workspace-legit-project')
    await fs.mkdir(projectDir, { recursive: true })

    // Write a legitimate file so we have at least one result to compare against.
    await fs.writeFile(
      path.join(projectDir, 'legit.jsonl'),
      `${makeUserLine('Legitimate question')}\n`,
      'utf8',
    )

    // A traversal filename like '../../evil.jsonl' cannot actually be written
    // as a directory entry with those literal characters on most filesystems
    // (the OS resolves the path separators).  Instead, verify the bounds-check
    // logic: if readdir somehow yielded a name that resolves outside root,
    // the guard rejects it.  We simulate this by writing a file one level up
    // from the project dir and confirming scanSessions never returns it.
    await fs.writeFile(
      path.join(projectsDir, 'escaped.jsonl'),
      `${makeUserLine('Should not appear')}\n`,
      'utf8',
    )

    const sessions = await scanSessions()

    // Only the legitimate file inside a project subdirectory should appear.
    expect(sessions).toHaveLength(1)
    expect(sessions[0]!.filename).toBe('legit')
  })

  it('deduplicates sessions by filename and keeps the newest entry', async () => {
    const projectsDir = path.join(tmpRoot, '.claude', 'projects')
    const olderProject = path.join(projectsDir, '-home-user-workspace-old-project')
    const newerProject = path.join(projectsDir, '-home-user-workspace-new-project')
    await fs.mkdir(olderProject, { recursive: true })
    await fs.mkdir(newerProject, { recursive: true })

    const sharedFilename = 'shared-session.jsonl'
    const olderFile = path.join(olderProject, sharedFilename)
    const newerFile = path.join(newerProject, sharedFilename)
    await fs.writeFile(olderFile, `${makeUserLine('old session')}\n`, 'utf8')
    await fs.writeFile(newerFile, `${makeUserLine('new session')}\n`, 'utf8')

    const olderTime = new Date('2026-03-01T00:00:00.000Z')
    const newerTime = new Date('2026-03-01T00:01:00.000Z')
    await fs.utimes(olderFile, olderTime, olderTime)
    await fs.utimes(newerFile, newerTime, newerTime)

    const sessions = await scanSessions()
    const matching = sessions.filter((s) => s.filename === 'shared-session')
    expect(matching).toHaveLength(1)
    expect(matching[0]!.absolutePath).toBe(newerFile)
  })
})

// ---------------------------------------------------------------------------
// cleanProjectName
// ---------------------------------------------------------------------------

describe('cleanProjectName', () => {
  it('drops suffix words (e.g. "rust")', () => {
    // "-home-jmagar-workspace-axon-rust" → parts: home, jmagar, workspace, axon, rust
    // last = "rust" (suffix word) → return prev = "axon"
    expect(cleanProjectName('-home-jmagar-workspace-axon-rust')).toBe('axon')
  })

  it('drops other known suffix words (src, git, main, master, rs)', () => {
    // Parts of '-home-user-my-project-src': ['home','user','my','project','src']
    // last='src' (suffix) → return prev which is 'project'
    expect(cleanProjectName('-home-user-my-project-src')).toBe('project')
    // Parts of '-repos-cool-thing-git': ['repos','cool','thing','git']
    // last='git' (suffix) → return prev which is 'thing'
    expect(cleanProjectName('-repos-cool-thing-git')).toBe('thing')
    // Parts of '-repos-my-lib-main': ['repos','my','lib','main']
    // last='main' (suffix) → return prev which is 'lib'
    expect(cleanProjectName('-repos-my-lib-main')).toBe('lib')
  })

  it('keeps meaningful last segment', () => {
    // last = "web" is not in SUFFIX_WORDS → return "app-web"
    expect(cleanProjectName('-home-user-workspace-app-web')).toBe('app-web')
  })

  it('returns single-segment name unchanged', () => {
    expect(cleanProjectName('myproject')).toBe('myproject')
  })

  it('handles no-hyphen input', () => {
    expect(cleanProjectName('nodash')).toBe('nodash')
  })

  it('handles leading dashes', () => {
    // After stripping leading dashes and splitting, parts = ['home', 'user', 'project']
    // last = "project" (not in SUFFIX_WORDS) → return "user-project"
    expect(cleanProjectName('-home-user-project')).toBe('user-project')
  })
})
