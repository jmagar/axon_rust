import { describe, expect, it } from 'vitest'
import {
  configToForm,
  type FormState,
  formToConfig,
  type KvPair,
} from '@/app/settings/mcp/mcp-types'

function kv(key: string, value: string): KvPair {
  return { id: 'test-id', key, value }
}

describe('formToConfig', () => {
  it('produces http config with url', () => {
    const form: FormState = {
      name: 'test',
      type: 'http',
      command: '',
      args: '',
      envPairs: [],
      url: 'http://localhost:3000/mcp',
      headerPairs: [],
    }
    expect(formToConfig(form)).toEqual({ url: 'http://localhost:3000/mcp' })
  })

  it('includes headers when non-empty keys exist', () => {
    const form: FormState = {
      name: 'test',
      type: 'http',
      command: '',
      args: '',
      envPairs: [],
      url: 'http://localhost:3000/mcp',
      headerPairs: [kv('Authorization', 'Bearer token')],
    }
    const cfg = formToConfig(form)
    expect(cfg.headers).toEqual({ Authorization: 'Bearer token' })
  })

  it('omits headers when all keys are empty', () => {
    const form: FormState = {
      name: 'test',
      type: 'http',
      command: '',
      args: '',
      envPairs: [],
      url: 'http://localhost:3000/mcp',
      headerPairs: [kv('', 'value'), kv('  ', 'value')],
    }
    const cfg = formToConfig(form)
    expect(cfg.headers).toBeUndefined()
  })

  it('trims header keys', () => {
    const form: FormState = {
      name: 'test',
      type: 'http',
      command: '',
      args: '',
      envPairs: [],
      url: 'http://localhost:3000/mcp',
      headerPairs: [kv('  X-Custom  ', 'val')],
    }
    const cfg = formToConfig(form)
    expect(cfg.headers).toEqual({ 'X-Custom': 'val' })
  })

  it('produces stdio config with command', () => {
    const form: FormState = {
      name: 'test',
      type: 'stdio',
      command: 'node',
      args: '',
      envPairs: [],
      url: '',
      headerPairs: [],
    }
    expect(formToConfig(form)).toEqual({ command: 'node' })
  })

  it('splits args by newline and trims', () => {
    const form: FormState = {
      name: 'test',
      type: 'stdio',
      command: 'node',
      args: 'server.js\n--port\n3000',
      envPairs: [],
      url: '',
      headerPairs: [],
    }
    const cfg = formToConfig(form)
    expect(cfg.args).toEqual(['server.js', '--port', '3000'])
  })

  it('filters empty lines from args', () => {
    const form: FormState = {
      name: 'test',
      type: 'stdio',
      command: 'node',
      args: 'a\n\n\nb',
      envPairs: [],
      url: '',
      headerPairs: [],
    }
    const cfg = formToConfig(form)
    expect(cfg.args).toEqual(['a', 'b'])
  })

  it('omits args when all empty', () => {
    const form: FormState = {
      name: 'test',
      type: 'stdio',
      command: 'node',
      args: '\n\n',
      envPairs: [],
      url: '',
      headerPairs: [],
    }
    const cfg = formToConfig(form)
    expect(cfg.args).toBeUndefined()
  })

  it('includes env when non-empty keys exist', () => {
    const form: FormState = {
      name: 'test',
      type: 'stdio',
      command: 'node',
      args: '',
      envPairs: [kv('NODE_ENV', 'production')],
      url: '',
      headerPairs: [],
    }
    const cfg = formToConfig(form)
    expect(cfg.env).toEqual({ NODE_ENV: 'production' })
  })

  it('omits env when all keys are empty', () => {
    const form: FormState = {
      name: 'test',
      type: 'stdio',
      command: 'node',
      args: '',
      envPairs: [kv('', 'value')],
      url: '',
      headerPairs: [],
    }
    const cfg = formToConfig(form)
    expect(cfg.env).toBeUndefined()
  })
})

describe('configToForm', () => {
  it('produces http form from url config', () => {
    const form = configToForm('my-server', { url: 'http://localhost:3000' })
    expect(form.name).toBe('my-server')
    expect(form.type).toBe('http')
    expect(form.url).toBe('http://localhost:3000')
    expect(form.command).toBe('')
  })

  it('produces stdio form from command config', () => {
    const form = configToForm('my-server', {
      command: 'node',
      args: ['server.js', '--port', '3000'],
    })
    expect(form.type).toBe('stdio')
    expect(form.command).toBe('node')
    expect(form.args).toBe('server.js\n--port\n3000')
  })

  it('maps env entries to kvPairs', () => {
    const form = configToForm('s', { command: 'x', env: { FOO: 'bar', BAZ: 'qux' } })
    expect(form.envPairs).toHaveLength(2)
    expect(form.envPairs[0].key).toBe('FOO')
    expect(form.envPairs[0].value).toBe('bar')
    expect(form.envPairs[1].key).toBe('BAZ')
    expect(form.envPairs[1].value).toBe('qux')
    // each pair should have a unique id
    expect(form.envPairs[0].id).not.toBe(form.envPairs[1].id)
  })

  it('maps header entries to kvPairs', () => {
    const form = configToForm('s', { url: 'http://x', headers: { Auth: 'Bearer t' } })
    expect(form.headerPairs).toHaveLength(1)
    expect(form.headerPairs[0].key).toBe('Auth')
    expect(form.headerPairs[0].value).toBe('Bearer t')
  })

  it('defaults missing fields', () => {
    const form = configToForm('s', {})
    expect(form.type).toBe('stdio')
    expect(form.command).toBe('')
    expect(form.args).toBe('')
    expect(form.url).toBe('')
    expect(form.envPairs).toEqual([])
    expect(form.headerPairs).toEqual([])
  })
})

describe('formToConfig → configToForm round-trip', () => {
  it('round-trips stdio config', () => {
    const original: FormState = {
      name: 'roundtrip',
      type: 'stdio',
      command: 'python',
      args: 'server.py\n--debug',
      envPairs: [kv('KEY', 'val')],
      url: '',
      headerPairs: [],
    }
    const cfg = formToConfig(original)
    const restored = configToForm('roundtrip', cfg)

    expect(restored.name).toBe(original.name)
    expect(restored.type).toBe(original.type)
    expect(restored.command).toBe(original.command)
    expect(restored.args).toBe(original.args)
    expect(restored.envPairs.map(({ key, value }) => ({ key, value }))).toEqual(
      original.envPairs.map(({ key, value }) => ({ key, value })),
    )
  })

  it('round-trips http config', () => {
    const original: FormState = {
      name: 'roundtrip',
      type: 'http',
      command: '',
      args: '',
      envPairs: [],
      url: 'https://api.example.com/mcp',
      headerPairs: [kv('X-Token', 'secret')],
    }
    const cfg = formToConfig(original)
    const restored = configToForm('roundtrip', cfg)

    expect(restored.type).toBe('http')
    expect(restored.url).toBe(original.url)
    expect(restored.headerPairs.map(({ key, value }) => ({ key, value }))).toEqual(
      original.headerPairs.map(({ key, value }) => ({ key, value })),
    )
  })
})
