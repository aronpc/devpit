import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { ContextMeter } from './ContextMeter'

describe('how full the context is', () => {
  it('shows nothing before a turn has measured it', () => {
    const { container } = render(<ContextMeter context={null} />)
    expect(container.textContent).toBe('')
  })

  it('says unavailable, never 0%, for an agent that does not report it', () => {
    render(<ContextMeter context={null} metered={false} />)
    expect(screen.getByText('n/a').title).toContain('does not report')
  })

  it('shows the share, and warns as it fills', () => {
    render(<ContextMeter context={{ used: 184_000, window: 200_000 }} />)
    const meter = screen.getByText('92%')
    expect(meter.dataset.level).toBe('full')
    expect(meter.title).toContain('184k of 200k')
    expect(meter.title).toContain('/compact')
  })
})
