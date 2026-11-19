import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { Host, HostLink } from '../gen/bindings'

let links: HostLink[]
let refusal: string | null
const link = vi.fn()
const unlink = vi.fn()

const hosts: Host[] = [
  { id: 'host_1', name: 'staging', target: 'stg', user: null, port: null, identityFile: null, workRoot: '/srv', environment: 'staging' },
  { id: 'host_2', name: 'web prod', target: 'prod', user: null, port: null, identityFile: null, workRoot: '/var/www', environment: 'production' },
]

vi.mock('./live', () => ({
  ask: async (call: () => Promise<{ data: unknown; error: string | null }>) => ({ ...(await call()), loading: false }),
  commands: {
    hostsList: async () => ({ data: { hosts }, error: null }),
    projectHosts: async () => ({ data: { links }, error: null }),
    projectHostLink: async (projectId: string, hostId: string, path: string) => {
      link(projectId, hostId, path)
      if (refusal) return { data: null, error: refusal }
      const host = hosts.find((one) => one.id === hostId)!
      links = [...links.filter((one) => one.hostId !== hostId), { hostId, hostName: host.name, environment: host.environment, path }]
      return { data: { links }, error: null }
    },
    projectHostUnlink: async (projectId: string, hostId: string) => {
      unlink(projectId, hostId)
      links = links.filter((one) => one.hostId !== hostId)
      return { data: { links }, error: null }
    },
  },
}))

import { ProjectHosts } from './ProjectHosts'

beforeEach(() => {
  links = [{ hostId: 'host_2', hostName: 'web prod', environment: 'production', path: '/var/www/demo' }]
  refusal = null
})

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

describe('Project → Hosts', () => {
  it('lists each link with its host and environment', async () => {
    render(<ProjectHosts projectId="prj_1" />)
    expect(await screen.findByText('/var/www/demo')).toBeTruthy()
    expect(screen.getByText('production', { selector: '.host__env' })).toBeTruthy()
  })

  it('links a host at a path the person gives', async () => {
    render(<ProjectHosts projectId="prj_1" />)
    await screen.findByText('/var/www/demo')
    fireEvent.change(screen.getByLabelText('Host to link'), { target: { value: 'host_1' } })
    fireEvent.change(screen.getByLabelText('Path on the host'), { target: { value: '/srv/demo' } })
    fireEvent.click(screen.getByRole('button', { name: 'Link' }))
    await waitFor(() => expect(link).toHaveBeenCalledWith('prj_1', 'host_1', '/srv/demo'))
    expect(await screen.findByText('/srv/demo')).toBeTruthy()
  })

  it('says why a link was refused', async () => {
    refusal = 'there is no folder /srv/demo on staging'
    render(<ProjectHosts projectId="prj_1" />)
    await screen.findByText('/var/www/demo')
    fireEvent.change(screen.getByLabelText('Host to link'), { target: { value: 'host_1' } })
    fireEvent.change(screen.getByLabelText('Path on the host'), { target: { value: '/srv/demo' } })
    fireEvent.click(screen.getByRole('button', { name: 'Link' }))
    expect((await screen.findByRole('alert')).textContent).toContain('no folder')
  })

  it('unlinks a host', async () => {
    render(<ProjectHosts projectId="prj_1" />)
    fireEvent.click(await screen.findByRole('button', { name: 'Unlink web prod' }))
    await waitFor(() => expect(unlink).toHaveBeenCalledWith('prj_1', 'host_2'))
    await waitFor(() => expect(screen.queryByText('/var/www/demo')).toBeNull())
  })
})
