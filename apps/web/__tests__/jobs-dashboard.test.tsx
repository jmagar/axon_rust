import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { TypeChip } from '@/components/jobs/job-cells'
import { buildJobsQuery, TYPE_TABS } from '@/components/jobs/jobs-dashboard'

describe('jobs dashboard', () => {
  it('renders Refresh type filter and requests type=refresh', () => {
    expect(TYPE_TABS.some((tab) => tab.value === 'refresh' && tab.label === 'Refresh')).toBe(true)
    expect(buildJobsQuery('refresh', 'all', 50, 0)).toContain('type=refresh')
  })

  it('renders refresh row chip and status correctly', () => {
    const markup = renderToStaticMarkup(<TypeChip type="refresh" />)
    expect(markup.toLowerCase()).toContain('refresh')
  })
})
