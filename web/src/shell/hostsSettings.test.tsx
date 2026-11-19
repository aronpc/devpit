import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { Host, HostDraft } from '../gen/bindings'

let hosts: Host[]
let refusal: string | null
const save = vi.fn()
const remove = vi.fn()

vi.mock('./live', () => ({
  ask: async (call: () => Promise<{ data: unknown; error: string | null }>) => {
    const answer = await call()
    return { ...answer, loading: false }
  },
  commands: {
    hostsList: async () => ({ data: { hosts }, error: null }),
    hostSave: async (draft: HostDraft) => {
      save(draft)
      if (refusal) return { data: null, error: refusal }
      const id = draft.id ?? `host_${hosts.length + 1}`
      const { id: _drop, ...rest } = draft
      hosts = [...hosts.filter((one) => one.id !== id), { ...rest, id }]
      return { data: { hosts }, error: null }
    },
    hostRemove: async (id: string) => {
      remove(id)
      hosts = hosts.filter((one) => one.id !== id)
      return { data: { hosts }, error: null }
    },
  },
}))

import { HostsSettings } from './HostsSettings'

const prod: Host = {
  id: 'host_1',
  name: 'web prod',
  target: 'prod-web',
  user: null,
  port: null,
  identityFile: null,
  workRoot: '/srv/app',
  environment: 'production',
}

beforeEach(() => {
  hosts = [prod]
  refusal = null
})

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

const type = (label: string, value: string): void => {
  fireEvent.change(screen.getByLabelText(label), { target: { value } })
}

describe('Settings → Hosts', () => {
  it('lists each saved host with its environment', async () => {
    render(<HostsSettings />)
    expect(await screen.findByText('web prod')).toBeTruthy()
    expect(screen.getByText('production', { selector: '.host__env' })).toBeTruthy()
  })

  it('adds a host from its fields, with the identity file as a path', async () => {
    render(<HostsSettings />)
    await screen.findByText('web prod')
    type('Host name', 'staging box')
    type('SSH target', 'staging-01')
    type('User', 'deploy')
    type('Port', '2222')
    type('Identity file', '~/.ssh/id_ed25519')
    type('Work root', '/srv/apps')
    fireEvent.change(screen.getByLabelText('Environment'), { target: { value: 'staging' } })
    fireEvent.click(screen.getByRole('button', { name: 'Add host' }))
    await waitFor(() =>
      expect(save).toHaveBeenCalledWith({
        id: null,
        name: 'staging box',
        target: 'staging-01',
        user: 'deploy',
        port: 2222,
        identityFile: '~/.ssh/id_ed25519',
        workRoot: '/srv/apps',
        environment: 'staging',
      }),
    )
    expect(await screen.findByText('staging box')).toBeTruthy()
  })

  it('says why a host was refused, and keeps what was typed', async () => {
    refusal = 'the SSH target is an ssh config alias or a host name'
    render(<HostsSettings />)
    await screen.findByText('web prod')
    type('Host name', 'odd')
    type('SSH target', 'a; b')
    type('Work root', '/srv')
    fireEvent.click(screen.getByRole('button', { name: 'Add host' }))
    expect((await screen.findByRole('alert')).textContent).toContain('ssh config alias')
    expect((screen.getByLabelText('SSH target') as HTMLInputElement).value).toBe('a; b')
  })

  it('sends a port out of range as one the backend refuses with its reason', async () => {
    render(<HostsSettings />)
    await screen.findByText('web prod')
    type('Host name', 'odd')
    type('SSH target', 'odd')
    type('Port', '70000')
    type('Work root', '/srv')
    fireEvent.click(screen.getByRole('button', { name: 'Add host' }))
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ port: 0 })))
  })

  it('edits a host in place', async () => {
    render(<HostsSettings />)
    fireEvent.click(await screen.findByRole('button', { name: 'Edit web prod' }))
    expect((screen.getByLabelText('SSH target') as HTMLInputElement).value).toBe('prod-web')
    fireEvent.change(screen.getByLabelText('Environment'), { target: { value: 'staging' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save host' }))
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ id: 'host_1', environment: 'staging' })))
  })

  it('removes a host', async () => {
    render(<HostsSettings />)
    fireEvent.click(await screen.findByRole('button', { name: 'Remove web prod' }))
    await waitFor(() => expect(remove).toHaveBeenCalledWith('host_1'))
    await waitFor(() => expect(screen.queryByText('web prod')).toBeNull())
  })
})
