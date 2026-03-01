import { describe, expect, it } from 'vitest'
import { makeInitialRuntimeState, reduceRuntimeState } from '@/hooks/use-ws-messages'
import type { WsServerMsg } from '@/lib/ws-protocol'

describe('use-ws-messages v2 lifecycle reducer', () => {
  it('tracks command.start and clears previous stdout payloads', () => {
    const withOutput = reduceRuntimeState(makeInitialRuntimeState(), {
      type: 'command.output.json',
      data: {
        ctx: {
          exec_id: 'exec-0',
          mode: 'extract',
          input: 'https://example.com',
        },
        data: { ok: true },
      },
    })

    const next = reduceRuntimeState(withOutput, {
      type: 'command.start',
      data: {
        ctx: {
          exec_id: 'exec-1',
          mode: 'extract',
          input: 'https://example.com',
        },
      },
    })

    expect(next.commandMode).toBe('extract')
    expect(next.stdoutJson).toEqual([])
  })

  it('projects v2 job.status and job.progress into lifecycle + stdout streams', () => {
    const stateWithJobId = reduceRuntimeState(makeInitialRuntimeState(), {
      type: 'command.output.json',
      data: {
        ctx: {
          exec_id: 'exec-2',
          mode: 'crawl',
          input: 'https://example.com',
        },
        data: { job_id: 'job-123', status: 'enqueued' },
      },
    })

    const afterStatus = reduceRuntimeState(stateWithJobId, {
      type: 'job.status',
      data: {
        ctx: {
          exec_id: 'exec-2',
          mode: 'crawl',
          input: 'https://example.com',
        },
        payload: {
          status: 'running',
          metrics: {
            pages_crawled: 2,
          },
        },
      },
    })

    const afterProgress = reduceRuntimeState(afterStatus, {
      type: 'job.progress',
      data: {
        ctx: {
          exec_id: 'exec-2',
          mode: 'crawl',
          input: 'https://example.com',
        },
        payload: {
          phase: 'fetching',
          percent: 42,
          processed: 84,
          total: 200,
        },
      },
    })

    expect(afterProgress.lifecycleEntries).toHaveLength(2)
    expect(afterProgress.lifecycleEntries[0]).toMatchObject({
      job_id: 'job-123',
      status: 'running',
      mode: 'crawl',
    })
    expect(afterProgress.lifecycleEntries[1]).toMatchObject({
      job_id: 'job-123',
      phase: 'fetching',
      percent: 42,
      processed: 84,
      total: 200,
      status: 'running',
    })
    expect(afterProgress.stdoutJson.slice(-2)).toEqual(afterProgress.lifecycleEntries)
  })

  it('derives currentJobId from metrics-only status before progress arrives', () => {
    const afterStatus = reduceRuntimeState(makeInitialRuntimeState(), {
      type: 'job.status',
      data: {
        ctx: {
          exec_id: 'exec-metrics-only',
          mode: 'crawl',
          input: 'https://example.com',
        },
        payload: {
          status: 'running',
          metrics: {
            job_id: 'job-metrics-only-1',
          },
        },
      },
    })

    const afterProgress = reduceRuntimeState(afterStatus, {
      type: 'job.progress',
      data: {
        ctx: {
          exec_id: 'exec-metrics-only',
          mode: 'crawl',
          input: 'https://example.com',
        },
        payload: {
          phase: 'fetching',
          percent: 10,
          processed: 1,
          total: 10,
        },
      },
    })

    expect(afterStatus.currentJobId).toBe('job-metrics-only-1')
    expect(afterProgress.currentJobId).toBe('job-metrics-only-1')
    expect(afterProgress.lifecycleEntries).toHaveLength(2)
    expect(afterProgress.lifecycleEntries[0]).toMatchObject({
      job_id: 'job-metrics-only-1',
      status: 'running',
      mode: 'crawl',
    })
    expect(afterProgress.lifecycleEntries[1]).toMatchObject({
      job_id: 'job-metrics-only-1',
      phase: 'fetching',
      percent: 10,
      processed: 1,
      total: 10,
      status: 'running',
    })
  })

  it('handles artifact.list and artifact.content v2 payloads', () => {
    const withArtifacts = reduceRuntimeState(makeInitialRuntimeState(), {
      type: 'artifact.list',
      data: {
        ctx: {
          exec_id: 'exec-3',
          mode: 'screenshot',
          input: 'https://example.com',
        },
        artifacts: [
          {
            path: '.cache/axon-rust/output/screenshots/example.png',
            download_url: '/download/job-1/file/example.png',
            size_bytes: 1024,
          },
        ],
      },
    })

    const withContent = reduceRuntimeState(withArtifacts, {
      type: 'artifact.content',
      data: {
        ctx: {
          exec_id: 'exec-3',
          mode: 'crawl',
          input: 'https://example.com',
        },
        path: 'index.md',
        content: '# hello',
      },
    })

    expect(withContent.screenshotFiles).toEqual([
      {
        path: '.cache/axon-rust/output/screenshots/example.png',
        name: 'example.png',
        serve_url: '/download/job-1/file/example.png',
        size_bytes: 1024,
      },
    ])
    expect(withContent.markdownContent).toBe('# hello')
  })

  it('keeps non-crawl progress mode-agnostic for extract and embed', () => {
    const modes = ['extract', 'embed']

    for (const mode of modes) {
      let state = makeInitialRuntimeState()
      state = reduceRuntimeState(state, {
        type: 'command.output.json',
        data: {
          ctx: {
            exec_id: `exec-${mode}`,
            mode,
            input: 'test-input',
          },
          data: { job_id: `${mode}-job-1`, status: 'enqueued' },
        },
      })

      state = reduceRuntimeState(state, {
        type: 'job.progress',
        data: {
          ctx: {
            exec_id: `exec-${mode}`,
            mode,
            input: 'test-input',
          },
          payload: {
            phase: 'processing',
            percent: 75,
            processed: 3,
            total: 4,
          },
        },
      })

      const lifecycleEntry = state.lifecycleEntries[state.lifecycleEntries.length - 1]
      expect(lifecycleEntry).toMatchObject({
        job_id: `${mode}-job-1`,
        mode,
        phase: 'processing',
        percent: 75,
        processed: 3,
        total: 4,
      })
    }
  })

  it('captures v2 cancel response payload for status UI', () => {
    const next = reduceRuntimeState(makeInitialRuntimeState(), {
      type: 'job.cancel.response',
      data: {
        ctx: {
          exec_id: 'exec-cancel',
          mode: 'extract',
          input: 'ignored',
        },
        payload: {
          ok: true,
          mode: 'extract',
          job_id: 'job-cancel-1',
          message: 'cancellation requested',
        },
      },
    } as WsServerMsg)

    expect(next.cancelResponse).toEqual({
      ok: true,
      mode: 'extract',
      job_id: 'job-cancel-1',
      message: 'cancellation requested',
    })
  })

  it('captures command.output.json payloads and detects current job id', () => {
    const next = reduceRuntimeState(makeInitialRuntimeState(), {
      type: 'command.output.json',
      data: {
        ctx: {
          exec_id: 'exec-sync-1',
          mode: 'query',
          input: 'hello',
        },
        data: { job_id: 'job-sync-1', rows: 3 },
      },
    } as WsServerMsg)

    expect(next.currentJobId).toBe('job-sync-1')
    expect(next.stdoutJson).toEqual([{ job_id: 'job-sync-1', rows: 3 }])
  })

  it('accepts command.done and command.error messages in reducer', () => {
    const done = reduceRuntimeState(makeInitialRuntimeState(), {
      type: 'command.done',
      data: {
        ctx: {
          exec_id: 'exec-done',
          mode: 'query',
          input: 'hello',
        },
        payload: {
          exit_code: 0,
          elapsed_ms: 42,
        },
      },
    } as WsServerMsg)

    const errored = reduceRuntimeState(done, {
      type: 'command.error',
      data: {
        ctx: {
          exec_id: 'exec-error',
          mode: 'query',
          input: 'hello',
        },
        payload: {
          message: 'boom',
          elapsed_ms: 43,
        },
      },
    } as WsServerMsg)

    expect(errored).toBeDefined()
    expect(errored.currentJobId).toBeNull()
  })
})
