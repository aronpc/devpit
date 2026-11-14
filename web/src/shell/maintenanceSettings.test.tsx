import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Maintenance } from '../gen/bindings'
import { MaintenanceSettings } from './MaintenanceSettings'

const GB = 1024 * 1024 * 1024
const before: Maintenance = {
  powerSaver: false,
  onBattery: false,
  keepDays: 0,
  projectAgent: true,
  projectAgentProjects: [
    { id: 'prj_api', name: 'api', on: true },
    { id: 'prj_web', name: 'web', on: true },
  ],
  canvas: true,
  last: null,
}
let stored: Maintenance = { ...before }
const write = vi.fn(async (powerSaver: boolean | null, keepDays: number | null, projectAgent: boolean | null, canvas: boolean | null) => {
  stored = {
    ...stored,
    powerSaver: powerSaver ?? stored.powerSaver,
    keepDays: keepDays ?? stored.keepDays,
    projectAgent: projectAgent ?? stored.projectAgent,
    canvas: canvas ?? stored.canvas,
  }
  return stored
})
const leave = vi.fn(async (projectId: string, on: boolean) => {
  stored = { ...stored, projectAgentProjects: stored.projectAgentProjects.map((one) => (one.id === projectId ? { ...one, on } : one)) }
  return stored
})
const cleanNow = vi.fn(async () => (stored = { ...stored, last: { at: Date.now() / 1000, removed: 3, freedBytes: 1.5 * GB } }))
vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => ({ data: await call(), error: null, loading: false }),
  commands: {
    maintenanceRead: async () => stored,
    maintenanceWrite: (powerSaver: boolean | null, keepDays: number | null, projectAgent: boolean | null, canvas: boolean | null) =>
      write(powerSaver, keepDays, projectAgent, canvas),
    maintenanceCleanNow: () => cleanNow(),
    maintenanceProjectAgent: (projectId: string, on: boolean) => leave(projectId, on),
  },
}))

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  stored = { ...before }
})

describe('Settings → Maintenance', () => {
  it('is balanced by default, and power saver turns on', async () => {
    render(<MaintenanceSettings />)
    const saver = await screen.findByRole('switch', { name: 'Power saver' })
    await waitFor(() => expect(saver.getAttribute('aria-checked')).toBe('false'))
    fireEvent.click(saver)
    await waitFor(() => expect(write).toHaveBeenLastCalledWith(true, null, null, null))
    await waitFor(() => expect(saver.getAttribute('aria-checked')).toBe('true'))
  })

  it('keeps everything until days are chosen, then deletes now and says what it freed', async () => {
    render(<MaintenanceSettings />)
    const days = (await screen.findByLabelText('Keep logs and run artifacts')) as HTMLSelectElement
    await waitFor(() => expect(days.value).toBe('0'))
    expect(screen.getByRole('option', { name: 'Until deleted' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Delete now' })).toBeNull()

    fireEvent.change(days, { target: { value: '30' } })
    await waitFor(() => expect(write).toHaveBeenLastCalledWith(null, 30, null, null))
    fireEvent.click(await screen.findByRole('button', { name: 'Delete now' }))
    await waitFor(() => expect(cleanNow).toHaveBeenCalled())
    expect(await screen.findByText('Last run just now: removed 3, freed 1.5 GB.')).toBeTruthy()
  })

  it('has Labs flags for the project agent and Canvas, on by default, each turning off', async () => {
    render(<MaintenanceSettings />)
    const agent = await screen.findByRole('switch', { name: 'Project agent' })
    const canvas = screen.getByRole('switch', { name: 'Canvas' })
    await waitFor(() => expect(agent.getAttribute('aria-checked')).toBe('true'))
    expect(canvas.getAttribute('aria-checked')).toBe('true')
    fireEvent.click(agent)
    await waitFor(() => expect(write).toHaveBeenLastCalledWith(null, null, false, null))
    fireEvent.click(canvas)
    await waitFor(() => expect(write).toHaveBeenLastCalledWith(null, null, null, false))
  })

  it('leaves one linked project out of the project agent, and only that one', async () => {
    render(<MaintenanceSettings />)
    const api = await screen.findByRole('switch', { name: 'Project agent in api' })
    expect(api.getAttribute('aria-checked')).toBe('true')
    fireEvent.click(api)
    await waitFor(() => expect(leave).toHaveBeenLastCalledWith('prj_api', false))
    await waitFor(() => expect(screen.getByRole('switch', { name: 'Project agent in api' }).getAttribute('aria-checked')).toBe('false'))
    expect(screen.getByRole('switch', { name: 'Project agent in web' }).getAttribute('aria-checked')).toBe('true')
  })
})
