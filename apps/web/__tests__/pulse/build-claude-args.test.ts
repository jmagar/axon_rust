import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { buildClaudeArgs } from '@/app/api/pulse/chat/claude-stream-types'
import { PulseChatRequestSchema } from '@/lib/pulse/types'

// ── helpers ──────────────────────────────────────────────────────────────────

const PROMPT = 'Hello, world'
const SYSTEM = 'You are helpful.'
const MODEL = 'sonnet' as const

/** Return the value that immediately follows a flag in the args array. */
function argAfter(args: string[], flag: string): string | undefined {
  const idx = args.indexOf(flag)
  return idx === -1 ? undefined : args[idx + 1]
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('buildClaudeArgs', () => {
  describe('core flags always present', () => {
    it('includes --output-format stream-json', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(argAfter(args, '--output-format')).toBe('stream-json')
    })

    it('includes --verbose flag', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).toContain('--verbose')
    })

    it('includes --system-prompt with the supplied value', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(argAfter(args, '--system-prompt')).toBe(SYSTEM)
    })

    it('includes --mcp-config', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).toContain('--mcp-config')
    })

    it('includes --strict-mcp-config flag', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).toContain('--strict-mcp-config')
    })

    it('includes --dangerously-skip-permissions flag', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).toContain('--dangerously-skip-permissions')
    })

    it('includes --include-partial-messages flag', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).toContain('--include-partial-messages')
    })

    it('passes the prompt as the value after -p', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(argAfter(args, '-p')).toBe(PROMPT)
    })
  })

  describe('effort', () => {
    it('defaults to medium when extra is omitted', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(argAfter(args, '--effort')).toBe('medium')
    })

    it('defaults to medium when extra.effort is undefined', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, {})
      expect(argAfter(args, '--effort')).toBe('medium')
    })

    it('uses high when extra.effort is high', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { effort: 'high' })
      expect(argAfter(args, '--effort')).toBe('high')
    })

    it('uses low when extra.effort is low', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { effort: 'low' })
      expect(argAfter(args, '--effort')).toBe('low')
    })
  })

  describe('model flag', () => {
    it('passes --model sonnet for sonnet', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, 'sonnet')
      expect(argAfter(args, '--model')).toBe('sonnet')
    })

    it('passes --model opus for opus', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, 'opus')
      expect(argAfter(args, '--model')).toBe('opus')
    })

    it('passes --model haiku for haiku', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, 'haiku')
      expect(argAfter(args, '--model')).toBe('haiku')
    })
  })

  describe('appendSystemPrompt', () => {
    it('appends --append-system-prompt when set', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, {
        appendSystemPrompt: 'Extra instructions.',
      })
      expect(argAfter(args, '--append-system-prompt')).toBe('Extra instructions.')
    })

    it('does not include --append-system-prompt when not set', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--append-system-prompt')
    })
  })

  describe('maxTurns', () => {
    it('appends --max-turns when maxTurns > 0', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { maxTurns: 5 })
      expect(argAfter(args, '--max-turns')).toBe('5')
    })

    it('does not include --max-turns when maxTurns is 0', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { maxTurns: 0 })
      expect(args).not.toContain('--max-turns')
    })

    it('does not include --max-turns when maxTurns is omitted', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--max-turns')
    })
  })

  describe('maxBudgetUsd', () => {
    it('appends --max-budget-usd when maxBudgetUsd > 0', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { maxBudgetUsd: 2.5 })
      expect(argAfter(args, '--max-budget-usd')).toBe('2.5')
    })

    it('does not include --max-budget-usd when maxBudgetUsd is 0', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { maxBudgetUsd: 0 })
      expect(args).not.toContain('--max-budget-usd')
    })

    it('does not include --max-budget-usd when omitted', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--max-budget-usd')
    })
  })

  describe('disableSlashCommands', () => {
    it('includes --disable-slash-commands when true', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { disableSlashCommands: true })
      expect(args).toContain('--disable-slash-commands')
    })

    it('does not include --disable-slash-commands when false', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { disableSlashCommands: false })
      expect(args).not.toContain('--disable-slash-commands')
    })

    it('does not include --disable-slash-commands when omitted', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--disable-slash-commands')
    })
  })

  describe('noSessionPersistence', () => {
    it('includes --no-session-persistence when true', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { noSessionPersistence: true })
      expect(args).toContain('--no-session-persistence')
    })

    it('does not include --no-session-persistence when false', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { noSessionPersistence: false })
      expect(args).not.toContain('--no-session-persistence')
    })
  })

  describe('fallbackModel', () => {
    it('appends --fallback-model when set', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { fallbackModel: 'haiku' })
      expect(argAfter(args, '--fallback-model')).toBe('haiku')
    })

    it('does not include --fallback-model when omitted', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--fallback-model')
    })
  })

  describe('allowedTools', () => {
    it('appends --allowedTools when set', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { allowedTools: 'Bash,Read' })
      expect(argAfter(args, '--allowedTools')).toBe('Bash,Read')
    })

    it('does not include --allowedTools when omitted', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--allowedTools')
    })

    it('strips shell injection after semicolon — Bash;rm -rf / emits only Bash', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { allowedTools: 'Bash;rm -rf /' })
      // TOOL_ENTRY_RE rejects "Bash;rm -rf /" as a single entry; "Bash" (before semicolon) is
      // not split on semicolons — the whole string is one comma-separated token, so the entire
      // value fails the regex and --allowedTools is omitted.
      const toolsValue = argAfter(args, '--allowedTools')
      // If anything appears, it must not contain the injection payload
      if (toolsValue !== undefined) {
        expect(toolsValue).not.toContain('rm')
        expect(toolsValue).not.toContain(';')
      }
    })

    it('strips subshell injection — Bash,$(malicious) emits only Bash', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { allowedTools: 'Bash,$(malicious)' })
      const toolsValue = argAfter(args, '--allowedTools')
      // $(malicious) fails TOOL_ENTRY_RE; only Bash survives the filter
      expect(toolsValue).toBe('Bash')
    })
  })

  describe('disallowedTools', () => {
    it('appends --disallowedTools when set', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { disallowedTools: 'Bash' })
      expect(argAfter(args, '--disallowedTools')).toBe('Bash')
    })

    it('does not include --disallowedTools when omitted', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--disallowedTools')
    })

    it('strips null-byte from tool entries — Bash\\x00null is filtered out', () => {
      // Split is on comma; the single entry "Bash\x00null" fails TOOL_ENTRY_RE
      // (null byte is not in the allowed character class), so it is dropped.
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, {
        disallowedTools: 'Bash\x00null',
      })
      // The poisoned entry is rejected, so --disallowedTools should be absent entirely
      expect(args).not.toContain('--disallowedTools')
    })
  })

  describe('addDir', () => {
    it('appends one --add-dir pair for a single allowed path', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { addDir: '/workspace' })
      const idx = args.indexOf('--add-dir')
      expect(idx).not.toBe(-1)
      expect(args[idx + 1]).toBe('/workspace')
    })

    it('appends multiple --add-dir pairs for comma-separated allowed paths', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { addDir: '/workspace,/tmp' })
      const dirs: string[] = []
      for (let i = 0; i < args.length - 1; i++) {
        if (args[i] === '--add-dir') {
          dirs.push(args[i + 1] as string)
        }
      }
      expect(dirs).toEqual(['/workspace', '/tmp'])
    })

    it('trims spaces around commas', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { addDir: ' /workspace , /tmp ' })
      const dirs: string[] = []
      for (let i = 0; i < args.length - 1; i++) {
        if (args[i] === '--add-dir') {
          dirs.push(args[i + 1] as string)
        }
      }
      expect(dirs).toEqual(['/workspace', '/tmp'])
    })

    it('skips empty segments from consecutive commas', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { addDir: '/workspace,,/tmp' })
      const dirs: string[] = []
      for (let i = 0; i < args.length - 1; i++) {
        if (args[i] === '--add-dir') {
          dirs.push(args[i + 1] as string)
        }
      }
      expect(dirs).toEqual(['/workspace', '/tmp'])
    })

    it('does not include --add-dir when omitted', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--add-dir')
    })

    it('rejects path traversal attempts — ../../etc/passwd must not appear in args', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { addDir: '../../etc/passwd' })
      // Neither --add-dir nor the resolved traversal path should be present
      expect(args).not.toContain('--add-dir')
      expect(args).not.toContain('/etc/passwd')
    })

    it('allows /home/node/workspace — valid subdirectory of an allowed root', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { addDir: '/home/node/workspace' })
      const idx = args.indexOf('--add-dir')
      expect(idx).not.toBe(-1)
      expect(args[idx + 1]).toBe('/home/node/workspace')
    })

    it('rejects /tmpevil — prefix match on /tmp must require a path separator boundary', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { addDir: '/tmpevil' })
      expect(args).not.toContain('--add-dir')
    })

    it('rejects /workspace-adjacent — not inside any allowed root', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { addDir: '/workspace-adjacent' })
      expect(args).not.toContain('--add-dir')
    })

    it('rejects /home/nodemodules — not inside /home/node', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { addDir: '/home/nodemodules' })
      expect(args).not.toContain('--add-dir')
    })
  })

  describe('betas', () => {
    it('appends --betas when set', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, {
        betas: 'interleaved-thinking',
      })
      expect(argAfter(args, '--betas')).toBe('interleaved-thinking')
    })

    it('does not include --betas when empty string', () => {
      // The implementation uses a truthy check, so '' is falsy — not appended.
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { betas: '' })
      expect(args).not.toContain('--betas')
    })

    it('does not include --betas when omitted', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--betas')
    })
  })

  describe('toolsRestrict', () => {
    it('appends --tools when toolsRestrict is set', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { toolsRestrict: 'Bash,Read' })
      expect(argAfter(args, '--tools')).toBe('Bash,Read')
    })

    it('does not include --tools when toolsRestrict is empty string', () => {
      // The implementation uses a truthy check — '' is falsy, so not appended.
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL, { toolsRestrict: '' })
      expect(args).not.toContain('--tools')
    })

    it('does not include --tools when omitted', () => {
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--tools')
    })
  })

  describe('PULSE_SKIP_PERMISSIONS env gate', () => {
    let originalValue: string | undefined

    beforeEach(() => {
      originalValue = process.env.PULSE_SKIP_PERMISSIONS
    })

    afterEach(() => {
      if (originalValue === undefined) {
        delete process.env.PULSE_SKIP_PERMISSIONS
      } else {
        process.env.PULSE_SKIP_PERMISSIONS = originalValue
      }
    })

    it('includes --dangerously-skip-permissions by default', () => {
      delete process.env.PULSE_SKIP_PERMISSIONS
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).toContain('--dangerously-skip-permissions')
    })

    it('omits --dangerously-skip-permissions when PULSE_SKIP_PERMISSIONS=false', () => {
      process.env.PULSE_SKIP_PERMISSIONS = 'false'
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).not.toContain('--dangerously-skip-permissions')
    })

    it('includes --dangerously-skip-permissions when PULSE_SKIP_PERMISSIONS=true', () => {
      process.env.PULSE_SKIP_PERMISSIONS = 'true'
      const args = buildClaudeArgs(PROMPT, SYSTEM, MODEL)
      expect(args).toContain('--dangerously-skip-permissions')
    })
  })
})

describe('PulseChatRequestSchema — sessionId validation', () => {
  const base = {
    prompt: 'Hello',
  }

  it('rejects sessionId that is too short (fewer than 8 hex/dash chars)', () => {
    const result = PulseChatRequestSchema.safeParse({ ...base, sessionId: 'abc' })
    expect(result.success).toBe(false)
  })

  it('accepts a short 8-char hex session hash', () => {
    const result = PulseChatRequestSchema.safeParse({ ...base, sessionId: 'abc12345' })
    expect(result.success).toBe(true)
  })

  it('accepts a full UUID-format sessionId', () => {
    const result = PulseChatRequestSchema.safeParse({
      ...base,
      sessionId: 'abc12345-1234-1234-1234-123456789012',
    })
    expect(result.success).toBe(true)
  })

  it('rejects sessionId containing invalid characters (path traversal attempt)', () => {
    const result = PulseChatRequestSchema.safeParse({
      ...base,
      sessionId: '../../../etc/passwd',
    })
    expect(result.success).toBe(false)
  })

  it('accepts sessionId as undefined (optional field)', () => {
    const result = PulseChatRequestSchema.safeParse({ ...base })
    expect(result.success).toBe(true)
    expect(result.data?.sessionId).toBeUndefined()
  })
})
