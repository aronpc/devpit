import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Project } from '../gen/bindings'

const edited = vi.fn()
const kept = vi.fn()
const rotation = vi.fn()
const settingsWritten = vi.fn()
const settings = { own: {}, repo: {}, machine: {}, applied: {}, repoProblem: null }
vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve(call()),
  commands: {
    projectEdit: (...args: unknown[]) => {
      edited(...args)
      return { data: { projects: [] }, error: null }
    },
    projectWorktreeSetup: () => ({ data: { copy: [], link: [], run: [], share: [] }, error: null }),
    projectWorktreeSetupSet: () => ({ data: null, error: null }),
    projectSettingsRead: () => ({ data: settings, error: null }),
    projectSettingsWrite: (...args: unknown[]) => {
      settingsWritten(...args)
      return { data: settings, error: null }
    },
    hostsList: () => ({ data: { hosts: [] }, error: null }),
    projectHosts: () => ({ data: { links: [] }, error: null }),
    projectSecrets: () => ({ data: { secrets: [], file: '/p/.env' }, error: null }),
    projectSecretSet: (...args: unknown[]) => {
      kept(...args)
      return { data: { secrets: [{ name: 'JEV_API_KEY', setAt: 1 }], file: '/p/.env' }, error: null }
    },
    projectSecretRotation: (...args: unknown[]) => {
      rotation(...args)
      return { data: { secrets: [{ name: 'JEV_API_KEY', setAt: 1, rotateDays: 90, rotateAt: 4e9 }], file: '/p/.env' }, error: null }
    },
  },
}))

const project = (over: Partial<Project> = {}): Project => ({
  id: 'p1',
  name: 'site',
  rootPath: '/home/me/site',
  group: null,
  accent: '#6f8fbf',
  worktrees: [],
  unreadable: null,
    orchestrator: null,
  origin: null,
  lastOpenedAt: null,
  icon: null,
  color: null,
  ...over,
})

const shell = {
  projects: [project(), project({ id: 'p2', name: 'api', group: 'Clients' })],
  reloadProjects: vi.fn(),
}
vi.mock('./useShell', () => ({ useShell: () => shell }))

import { ProjectDialog } from './ProjectDialog'

afterEach(() => {
  cleanup()
  edited.mockReset()
  settingsWritten.mockReset()
})

describe('the project dialog', () => {
  it('saves the name, group, icon and colour together', async () => {
    const close = vi.fn()
    render(<ProjectDialog project={project()} onClose={close} />)
    fireEvent.change(screen.getByDisplayValue('site'), { target: { value: 'Site' } })
    fireEvent.click(screen.getByText('No group'))
    fireEvent.click(screen.getByRole('option', { name: 'Clients' }))
    fireEvent.click(screen.getByTitle('rocket'))
    fireEvent.click(screen.getByTitle('#62c987'))
    fireEvent.click(screen.getByText('Save'))
    await waitFor(() => expect(close).toHaveBeenCalled())
    expect(edited).toHaveBeenCalledWith('p1', 'Site', 'Clients', 'icon:rocket', '#62c987')
    expect(shell.reloadProjects).toHaveBeenCalled()
  })

  it('takes an emoji as the icon', async () => {
    render(<ProjectDialog project={project()} onClose={vi.fn()} />)
    fireEvent.change(screen.getByPlaceholderText('…or an emoji'), { target: { value: '🚀' } })
    fireEvent.click(screen.getByText('Save'))
    await waitFor(() => expect(edited).toHaveBeenCalledWith('p1', 'site', null, '🚀', null))
  })

  it('keeps automatic account switching off until this project turns it on', async () => {
    render(<ProjectDialog project={project()} onClose={vi.fn()} />)
    const toggle = await screen.findByRole('switch', { name: 'Switch accounts automatically' })
    expect(toggle.getAttribute('aria-checked')).toBe('false')

    fireEvent.click(toggle)
    expect(toggle.getAttribute('aria-checked')).toBe('true')
    fireEvent.click(screen.getByText('Save'))

    await waitFor(() => expect(settingsWritten).toHaveBeenCalledWith('p1', 'automaticSwitch', 'true'))
  })

  it('offers the groups other projects are in, and a new one', () => {
    render(<ProjectDialog project={project()} onClose={vi.fn()} />)
    fireEvent.click(screen.getByText('No group'))
    expect(screen.getAllByRole('option').map((one) => one.textContent)).toEqual(['No group', 'Clients'])
    fireEvent.click(screen.getByText('New group…'))
    const typed = screen.getByPlaceholderText('Group name')
    fireEvent.change(typed, { target: { value: 'Side' } })
    fireEvent.keyDown(typed, { key: 'Enter' })
    expect(screen.getByText('Side')).toBeTruthy()
  })

  it('keeps a secret from a password field, by its name in capitals', async () => {
    render(<ProjectDialog project={project()} onClose={vi.fn()} />)
    fireEvent.change(screen.getByPlaceholderText('JEV_API_KEY'), { target: { value: 'jev_api_key' } })
    const value = screen.getByLabelText('Value') as HTMLInputElement
    expect(value.type).toBe('password')
    fireEvent.change(value, { target: { value: 'sk-abc' } })
    fireEvent.click(screen.getByText('Keep it'))
    await waitFor(() => expect(kept).toHaveBeenCalledWith('p1', 'JEV_API_KEY', 'sk-abc'))
    await waitFor(() => expect(value.value).toBe(''))
  })

  it('reminds to rotate a kept secret every so many days', async () => {
    render(<ProjectDialog project={project()} onClose={vi.fn()} />)
    fireEvent.change(screen.getByPlaceholderText('JEV_API_KEY'), { target: { value: 'JEV_API_KEY' } })
    fireEvent.change(screen.getByLabelText('Value'), { target: { value: 'sk-abc' } })
    fireEvent.click(screen.getByText('Keep it'))
    const picker = await waitFor(() => screen.getByLabelText('Rotation of JEV_API_KEY'))
    fireEvent.change(picker, { target: { value: '90' } })
    await waitFor(() => expect(rotation).toHaveBeenCalledWith('p1', 'JEV_API_KEY', 90, null))
    await waitFor(() => expect(screen.getByText(/rotate by/)).toBeTruthy())
  })
})
