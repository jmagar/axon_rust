import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { RunAxonCommandWsStreamOptions } from '@/lib/axon-ws-exec'

let pendingScenario:
  | ((args: { mode: string; options: RunAxonCommandWsStreamOptions }) => void | Promise<void>)
  | null = null
let wsRunSpy = vi.fn()

function queueScenario(
  scenario: (args: {
    mode: string
    options: RunAxonCommandWsStreamOptions
  }) => void | Promise<void>,
): void {
  pendingScenario = scenario
}

describe('pulse config probe route', () => {
  beforeEach(() => {
    vi.resetModules()
    pendingScenario = null
    wsRunSpy = vi.fn()

    vi.doMock('@/lib/axon-ws-exec', () => ({
      runAxonCommandWsStream: (
        mode: string,
        options: RunAxonCommandWsStreamOptions = {},
      ): Promise<void> => {
        wsRunSpy(mode, options)
        if (!pendingScenario) {
          throw new Error('Missing WS scenario for test')
        }
        const scenario = pendingScenario
        pendingScenario = null
        return Promise.resolve(scenario({ mode, options }))
      },
    }))
  })

  it('returns codex config options from probe updates', async () => {
    queueScenario(({ options }) => {
      queueMicrotask(() => {
        options.onJson?.({
          type: 'config_option_update',
          configOptions: [
            {
              id: 'model',
              name: 'Model',
              category: 'model',
              currentValue: 'gpt-5.3-codex',
              options: [
                { value: 'gpt-5.3-codex', name: 'GPT 5.3 Codex' },
                { value: 'gpt-5.4', name: 'GPT 5.4' },
              ],
            },
          ],
        })
        options.onDone?.({ exit_code: 0 })
      })
    })

    const mod = await import('@/app/api/pulse/config/route')
    const req = new Request('http://localhost/api/pulse/config', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ agent: 'codex', model: 'default' }),
    })

    const res = await mod.POST(req)
    expect(res.status).toBe(200)
    await expect(res.json()).resolves.toEqual({
      configOptions: [
        {
          id: 'model',
          name: 'Model',
          category: 'model',
          currentValue: 'gpt-5.3-codex',
          options: [
            { value: 'gpt-5.3-codex', name: 'GPT 5.3 Codex' },
            { value: 'gpt-5.4', name: 'GPT 5.4' },
          ],
        },
      ],
    })

    const wsOptions = wsRunSpy.mock.calls[0]?.[1] as RunAxonCommandWsStreamOptions
    expect(wsRunSpy.mock.calls[0]?.[0]).toBe('pulse_chat_probe')
    expect(wsOptions.flags?.agent).toBe('codex')
    expect(wsOptions.flags?.model).toBeUndefined()
  })

  it('returns empty config options for non-codex agents', async () => {
    const mod = await import('@/app/api/pulse/config/route')
    const req = new Request('http://localhost/api/pulse/config', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ agent: 'claude' }),
    })

    const res = await mod.POST(req)
    expect(res.status).toBe(200)
    await expect(res.json()).resolves.toEqual({ configOptions: [] })
    expect(wsRunSpy).not.toHaveBeenCalled()
  })
})
