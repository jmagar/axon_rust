import { describe, expect, it } from 'vitest'
import {
  buildJobDetailRequestPath,
  shouldRefetchArtifactsOnTerminalTransition,
} from '@/app/jobs/[id]/page'

describe('job detail page polling behavior', () => {
  it('re-requests includeArtifacts=1 after status transitions from running to completed', () => {
    expect(shouldRefetchArtifactsOnTerminalTransition('running', 'completed')).toBe(true)
    expect(buildJobDetailRequestPath('abc', true)).toBe('/api/jobs/abc?includeArtifacts=1')
  })

  it('does not trigger artifact refetch for non-terminal transitions', () => {
    expect(shouldRefetchArtifactsOnTerminalTransition('running', 'running')).toBe(false)
    expect(shouldRefetchArtifactsOnTerminalTransition('pending', 'running')).toBe(false)
  })
})
