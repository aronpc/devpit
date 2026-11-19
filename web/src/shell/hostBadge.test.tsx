import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { HostOfTab } from '../gen/bindings'

let answers: (HostOfTab | null | 'error')[]
const asked = vi.fn()

vi.mock('./live', () => ({
  ask: async (call: () => Promise<{ data: unknown; error: string | null }>) => ({ ...(await call()), loading: false }),
  commands: {
    tabHost: async (projectId: string, tabId: string) => {
      asked(projectId, tabId)
      const host = answers.length > 1 ? answers.shift()! : answers[0]
      return host === 'error' ? { data: null, error: 'the store is busy' } : { data: { host }, error: null }
    },
  },
}))

import { HostBadge } from './HostBadge'

const staging: HostOfTab = { hostId: 'host_1', name: 'staging box', environment: 'staging', connected: true }

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

describe('the host badge on a tab', () => {
  it('names the host and its environment for a tab on a host', async () => {
    answers = [staging]
    render(<HostBadge projectId="prj_1" tabId="tab_1" />)
    const badge = await screen.findByText(/staging box/)
    expect(badge.textContent).toContain('staging')
    expect(asked).toHaveBeenCalledWith('prj_1', 'tab_1')
  })

  it('shows nothing for a tab on this computer', async () => {
    answers = [null]
    const { container } = render(<HostBadge projectId="prj_1" tabId="tab_1" />)
    await waitFor(() => expect(asked).toHaveBeenCalled())
    expect(container.textContent).toBe('')
  })

  it('reads PROD, in red, for a production host', async () => {
    answers = [{ ...staging, name: 'web', environment: 'production' }]
    render(<HostBadge projectId="prj_1" tabId="tab_1" />)
    const badge = await screen.findByText(/PROD/)
    expect(badge.closest('[data-env]')?.getAttribute('data-env')).toBe('production')
  })

  it('turns disconnected when the connection drops, and back once it is up', async () => {
    answers = [staging, { ...staging, connected: false }, staging]
    render(<HostBadge projectId="prj_1" tabId="tab_1" every={30} />)
    await screen.findByText(/staging box/)
    await waitFor(() => expect(screen.getByLabelText(/disconnected/)).toBeTruthy())
    await waitFor(() => expect(screen.queryByLabelText(/disconnected/)).toBeNull())
  })
})

describe('the host badge, when asking fails', () => {
  it('keeps what it showed, PROD included, and asks again', async () => {
    answers = [{ ...staging, name: 'web', environment: 'production' }, 'error', 'error', 'error']
    render(<HostBadge projectId="prj_1" tabId="tab_1" every={30} />)
    await screen.findByText(/PROD/)
    await waitFor(() => expect(asked.mock.calls.length).toBeGreaterThanOrEqual(5))
    expect(screen.getByText(/PROD/)).toBeTruthy()
  })

})

describe('the host badge, laid out', () => {
  it('puts the environment before a long name, so the name is what gets cut', async () => {
    answers = [{ ...staging, name: 'payments-api-primary-eu-west-1-blue', environment: 'production' }]
    render(<HostBadge projectId="prj_1" tabId="tab_1" />)
    const env = await screen.findByText('PROD')
    const name = screen.getByText('payments-api-primary-eu-west-1-blue')
    expect(env.compareDocumentPosition(name) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(name.className).toBe('tab__hostn')
  })
})

describe('the host badge, as a tab changes', () => {
  it('asks again once its panes change, as a join into the tab does', async () => {
    answers = [null]
    const { rerender } = render(<HostBadge projectId="prj_1" tabId="tab_1" shape="leaf_a" />)
    await waitFor(() => expect(asked).toHaveBeenCalledTimes(1))
    answers = [{ ...staging, name: 'web', environment: 'production' }]
    rerender(<HostBadge projectId="prj_1" tabId="tab_1" shape="leaf_a,leaf_b" />)
    expect(await screen.findByText('PROD')).toBeTruthy()
  })
})

describe('the host badge, while a tab changes', () => {
  it('keeps PROD in view while a split asks again, and when that look fails', async () => {
    answers = [{ ...staging, name: 'web', environment: 'production' }]
    const { rerender } = render(<HostBadge projectId="prj_1" tabId="tab_1" shape="leaf_a" />)
    await screen.findByText('PROD')
    answers = ['error']
    rerender(<HostBadge projectId="prj_1" tabId="tab_1" shape="leaf_a,leaf_b" />)
    await waitFor(() => expect(asked).toHaveBeenCalledTimes(2))
    expect(screen.getByText('PROD')).toBeTruthy()
  })
})
