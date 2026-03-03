import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// We need to test parseDotenvLine which is not exported, so we'll test ensureRepoRootEnvLoaded
// behavior end-to-end. But first, let's extract the parser logic via dynamic import trickery.
// Since parseDotenvLine is private, we test it through ensureRepoRootEnvLoaded's behavior.

// Mock fs and workspace-root before importing the module
vi.mock('node:fs', () => ({
  default: {
    existsSync: vi.fn(),
    readFileSync: vi.fn(),
  },
}))

vi.mock('@/lib/pulse/workspace-root', () => ({
  getWorkspaceRoot: vi.fn(() => '/mock/repo'),
}))

describe('ensureRepoRootEnvLoaded', () => {
  let fs: { existsSync: ReturnType<typeof vi.fn>; readFileSync: ReturnType<typeof vi.fn> }
  let ensureRepoRootEnvLoaded: () => void
  const savedEnv: Record<string, string | undefined> = {}

  beforeEach(async () => {
    // Reset module state between tests (rootEnvLoaded is module-level)
    vi.resetModules()

    const fsMod = await import('node:fs')
    fs = fsMod.default as any

    const mod = await import('@/lib/pulse/server-env')
    ensureRepoRootEnvLoaded = mod.ensureRepoRootEnvLoaded
  })

  afterEach(() => {
    // Restore any env vars we set
    for (const key of Object.keys(savedEnv)) {
      if (savedEnv[key] === undefined) {
        delete process.env[key]
      } else {
        process.env[key] = savedEnv[key]
      }
    }
  })

  function setEnvFile(content: string) {
    fs.existsSync.mockReturnValue(true)
    fs.readFileSync.mockReturnValue(content)
  }

  function trackEnv(key: string) {
    savedEnv[key] = process.env[key]
  }

  it('loads key=value pairs into process.env', () => {
    trackEnv('TEST_SERVER_ENV_A')
    setEnvFile('TEST_SERVER_ENV_A=hello')

    ensureRepoRootEnvLoaded()

    expect(process.env.TEST_SERVER_ENV_A).toBe('hello')
  })

  it('strips surrounding quotes (double)', () => {
    trackEnv('TEST_SERVER_ENV_B')
    setEnvFile('TEST_SERVER_ENV_B="quoted value"')

    ensureRepoRootEnvLoaded()

    expect(process.env.TEST_SERVER_ENV_B).toBe('quoted value')
  })

  it('strips surrounding quotes (single)', () => {
    trackEnv('TEST_SERVER_ENV_C')
    setEnvFile("TEST_SERVER_ENV_C='single quoted'")

    ensureRepoRootEnvLoaded()

    expect(process.env.TEST_SERVER_ENV_C).toBe('single quoted')
  })

  it('skips comments and blank lines', () => {
    trackEnv('TEST_SERVER_ENV_D')
    setEnvFile('# comment\n\nTEST_SERVER_ENV_D=value\n  \n# another comment')

    ensureRepoRootEnvLoaded()

    expect(process.env.TEST_SERVER_ENV_D).toBe('value')
  })

  it('does not overwrite existing env vars', () => {
    trackEnv('TEST_SERVER_ENV_E')
    process.env.TEST_SERVER_ENV_E = 'original'
    setEnvFile('TEST_SERVER_ENV_E=overwritten')

    ensureRepoRootEnvLoaded()

    expect(process.env.TEST_SERVER_ENV_E).toBe('original')
  })

  it('handles missing .env file', () => {
    fs.existsSync.mockReturnValue(false)

    expect(() => ensureRepoRootEnvLoaded()).not.toThrow()
  })

  it('handles unreadable .env file', () => {
    fs.existsSync.mockReturnValue(true)
    fs.readFileSync.mockImplementation(() => {
      throw new Error('EACCES')
    })

    expect(() => ensureRepoRootEnvLoaded()).not.toThrow()
  })

  it('only loads once (idempotent)', () => {
    trackEnv('TEST_SERVER_ENV_F')
    setEnvFile('TEST_SERVER_ENV_F=first')

    ensureRepoRootEnvLoaded()
    expect(process.env.TEST_SERVER_ENV_F).toBe('first')

    // Change file content and call again — should not reload
    setEnvFile('TEST_SERVER_ENV_F=second')
    delete process.env.TEST_SERVER_ENV_F
    ensureRepoRootEnvLoaded()

    // Still undefined because module won't re-read
    expect(process.env.TEST_SERVER_ENV_F).toBeUndefined()
  })

  it('handles lines without = sign', () => {
    trackEnv('TEST_SERVER_ENV_G')
    setEnvFile('INVALID_LINE\nTEST_SERVER_ENV_G=valid')

    ensureRepoRootEnvLoaded()

    expect(process.env.TEST_SERVER_ENV_G).toBe('valid')
  })

  it('handles values with = signs', () => {
    trackEnv('TEST_SERVER_ENV_H')
    setEnvFile('TEST_SERVER_ENV_H=postgresql://user:pass@host/db?opt=val')

    ensureRepoRootEnvLoaded()

    expect(process.env.TEST_SERVER_ENV_H).toBe('postgresql://user:pass@host/db?opt=val')
  })
})
