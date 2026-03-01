import { describe, expect, it } from 'vitest'
import type {
  WsLifecycleEntry,
  WsServerMsg,
  WsV2ArtifactContentMsg,
  WsV2ArtifactListMsg,
  WsV2CommandContext,
  WsV2CommandDoneMsg,
  WsV2CommandErrorMsg,
  WsV2CommandOutputJsonMsg,
  WsV2CommandOutputLineMsg,
  WsV2CommandStartMsg,
  WsV2JobProgressMsg,
  WsV2JobStatusMsg,
} from '@/lib/ws-protocol'
import { lifecycleFromJobProgress, lifecycleFromJobStatus } from '@/lib/ws-protocol'

describe('ws protocol v2 message shapes', () => {
  const ctx: WsV2CommandContext = {
    exec_id: 'exec-123',
    mode: 'crawl',
    input: 'https://example.com',
  }

  it('models command.start with shared ctx inside data', () => {
    const message: WsV2CommandStartMsg = {
      type: 'command.start',
      data: {
        ctx,
      },
    }
    const serverMessage: WsServerMsg = message

    expect(serverMessage).toEqual({
      type: 'command.start',
      data: { ctx },
    })
  })

  it('models job.status payload including metrics and optional error', () => {
    const message: WsV2JobStatusMsg = {
      type: 'job.status',
      data: {
        ctx,
        payload: {
          status: 'running',
          error: 'none',
          metrics: {
            pages_crawled: 2,
            thin_pages: 0,
          },
        },
      },
    }
    const serverMessage: WsServerMsg = message

    expect(serverMessage).toEqual({
      type: 'job.status',
      data: {
        ctx,
        payload: {
          status: 'running',
          error: 'none',
          metrics: {
            pages_crawled: 2,
            thin_pages: 0,
          },
        },
      },
    })
  })

  it('models job.progress payload with optional counters omitted', () => {
    const message: WsV2JobProgressMsg = {
      type: 'job.progress',
      data: {
        ctx,
        payload: {
          phase: 'fetching',
          percent: 25,
        },
      },
    }

    expect(message.data.payload.phase).toBe('fetching')
    expect(message.data.payload.percent).toBe(25)
    expect(message.data.payload.processed).toBeUndefined()
    expect(message.data.payload.total).toBeUndefined()
  })

  it('models command.output.json payload shape', () => {
    const message: WsV2CommandOutputJsonMsg = {
      type: 'command.output.json',
      data: {
        ctx,
        data: { ok: true },
      },
    }
    const serverMessage: WsServerMsg = message
    expect(serverMessage).toEqual({
      type: 'command.output.json',
      data: { ctx, data: { ok: true } },
    })
  })

  it('models command.output.line payload shape', () => {
    const message: WsV2CommandOutputLineMsg = {
      type: 'command.output.line',
      data: {
        ctx,
        line: 'processing...',
      },
    }
    const serverMessage: WsServerMsg = message
    expect(serverMessage).toEqual({
      type: 'command.output.line',
      data: { ctx, line: 'processing...' },
    })
  })

  it('models artifact.list entries with optional fields', () => {
    const message: WsV2ArtifactListMsg = {
      type: 'artifact.list',
      data: {
        ctx,
        artifacts: [
          {
            kind: 'screenshot',
            path: 'output/report.png',
            download_url: '/download/job-1/file/output/report.png',
            mime: 'image/png',
            size_bytes: 1024,
          },
          {
            path: 'output/summary.md',
          },
        ],
      },
    }

    expect(message.data.artifacts).toHaveLength(2)
    expect(message.data.artifacts[0]).toMatchObject({
      kind: 'screenshot',
      size_bytes: 1024,
    })
    expect(message.data.artifacts[1]).toEqual({
      path: 'output/summary.md',
    })
  })

  it('models artifact.content payload shape', () => {
    const message: WsV2ArtifactContentMsg = {
      type: 'artifact.content',
      data: {
        ctx,
        path: 'output/summary.md',
        content: '# Summary',
      },
    }

    const serverMessage: WsServerMsg = message
    expect(serverMessage).toEqual({
      type: 'artifact.content',
      data: {
        ctx,
        path: 'output/summary.md',
        content: '# Summary',
      },
    })
  })

  it('models command.done payload shape', () => {
    const message: WsV2CommandDoneMsg = {
      type: 'command.done',
      data: {
        ctx,
        payload: {
          exit_code: 0,
          elapsed_ms: 123,
        },
      },
    }
    const serverMessage: WsServerMsg = message
    expect(serverMessage).toEqual({
      type: 'command.done',
      data: {
        ctx,
        payload: {
          exit_code: 0,
          elapsed_ms: 123,
        },
      },
    })
  })

  it('models command.error payload shape', () => {
    const message: WsV2CommandErrorMsg = {
      type: 'command.error',
      data: {
        ctx,
        payload: {
          message: 'boom',
          elapsed_ms: 88,
        },
      },
    }
    const serverMessage: WsServerMsg = message
    expect(serverMessage).toEqual({
      type: 'command.error',
      data: {
        ctx,
        payload: {
          message: 'boom',
          elapsed_ms: 88,
        },
      },
    })
  })
})

describe('ws protocol v2 lifecycle helpers', () => {
  const ctx: WsV2CommandContext = {
    exec_id: 'exec-999',
    mode: 'embed',
    input: 'docs/',
  }

  it('maps job.status to lifecycle entry with fallback job id', () => {
    const lifecycle = lifecycleFromJobStatus(
      {
        type: 'job.status',
        data: {
          ctx,
          payload: {
            status: 'running',
          },
        },
      },
      'job-abc',
    )

    expect(lifecycle).toEqual<WsLifecycleEntry>({
      job_id: 'job-abc',
      mode: 'embed',
      status: 'running',
      error_text: undefined,
    })
  })

  it('maps job.progress to lifecycle entry with counters', () => {
    const lifecycle = lifecycleFromJobProgress(
      {
        type: 'job.progress',
        data: {
          ctx,
          payload: {
            phase: 'processing',
            percent: 60,
            processed: 6,
            total: 10,
          },
        },
      },
      'job-abc',
    )

    expect(lifecycle).toEqual<WsLifecycleEntry>({
      job_id: 'job-abc',
      mode: 'embed',
      status: 'running',
      phase: 'processing',
      percent: 60,
      processed: 6,
      total: 10,
    })
  })
})
