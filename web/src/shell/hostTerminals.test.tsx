import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { HostLink } from '../gen/bindings'

let links: HostLink[]
let refusal: string | null
const opened = vi.fn()

vi.mock('./useShell', () => ({ useShell: () => ({ project: { id: 'prj_1', name: 'site' } }) }))
vi.mock('./live', () => ({
  ask: async (call: () => Promise<{ data: unknown; error: string | null }>) => ({ ...(await call()), loading: false }),
  commands: {
    projectHosts: async () => ({ data: { links }, error: null }),
    hostTerminalOpen: async (projectId: string, hostId: string) => {
      opened(projectId, hostId)
      return refusal ? { data: null, error: refusal } : { data: { tabId: 'tab_1', paneId: 'leaf_1' }, error: null }
    },
  },
}))

import { HostTerminals } from './HostTerminals'
import { PROBLEM } from './problems'

beforeEach(() => {
  links = [
    { hostId: 'host_1', hostName: 'staging', environment: 'staging', path: '/srv/site' },
    { hostId: 'host_2', hostName: 'web prod', environment: 'production', path: '/var/www/site' },
  ]
  refusal = null
})

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

describe('a new terminal on a host', () => {
  it('offers one for each host the project is linked to, and opens it there', async () => {
    render(<HostTerminals />)
    fireEvent.click(await screen.findByRole('button', { name: 'New terminal on a host' }))
    fireEvent.click(screen.getByRole('menuitem', { name: /web prod/ }))
    await waitFor(() => expect(opened).toHaveBeenCalledWith('prj_1', 'host_2'))
    expect(screen.queryByRole('menuitem')).toBeNull()
  })

  it('offers nothing for a project linked to no host', async () => {
    links = []
    render(<HostTerminals />)
    await waitFor(() => expect(screen.queryByRole('button', { name: 'New terminal on a host' })).toBeNull())
  })

  it('says why a terminal could not open there, where refusals are said', async () => {
    refusal = 'ssh could not reach staging: Connection timed out'
    const said: string[] = []
    const heard = (event: Event): void => void said.push((event as CustomEvent<string>).detail)
    window.addEventListener(PROBLEM, heard)
    render(<HostTerminals />)
    fireEvent.click(await screen.findByRole('button', { name: 'New terminal on a host' }))
    fireEvent.click(screen.getByRole('menuitem', { name: /staging/ }))
    await waitFor(() => expect(said).toContain('ssh could not reach staging: Connection timed out'))
    window.removeEventListener(PROBLEM, heard)
  })

  it('closes its menu on Escape', async () => {
    render(<HostTerminals />)
    fireEvent.click(await screen.findByRole('button', { name: 'New terminal on a host' }))
    expect(screen.getByRole('menu')).toBeTruthy()
    fireEvent.keyDown(window, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
  })
})
