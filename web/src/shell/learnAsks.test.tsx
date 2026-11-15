import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { LabsProject, Maintenance } from '../gen/bindings'
import { LearnAsks } from './LearnAsks'

const heard: Record<string, (payload: unknown) => void> = {}
vi.mock('./window', () => ({
  onCarried: (name: string, then: (payload: unknown) => void) => {
    heard[name] = then
    return () => delete heard[name]
  },
}))

const linked = (id: string, asked: boolean): LabsProject => ({ id, name: id.replace('prj_', ''), on: false, acceptFacts: false, asked })

let projects: LabsProject[] = []
let labs = true
const answered = vi.fn()
const shown = (): Maintenance => ({
  powerSaver: false,
  onBattery: false,
  keepDays: 0,
  projectAgent: labs,
  projectAgentProjects: projects,
  canvas: true,
  last: null,
})
vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => {
    try {
      return { data: await call(), error: null, loading: false }
    } catch (why) {
      return { data: null, error: String(why), loading: false }
    }
  },
  commands: {
    maintenanceRead: async (): Promise<Maintenance> => shown(),
    maintenanceProjectAgent: async (id: string, on: boolean): Promise<Maintenance> => {
      answered(id, on)
      projects = projects.map((one) => (one.id === id ? { ...one, on, asked: false } : one))
      return shown()
    },
  },
}))

afterEach(() => {
  cleanup()
  projects = []
  labs = true
  answered.mockReset()
})

describe('LearnAsks', () => {
  it('draws nothing while no project waits for an answer', async () => {
    projects = [linked('prj_api', false)]
    render(<LearnAsks />)
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull())
  })

  it('asks before a linked project learns, and turns it on only on the click', async () => {
    projects = [linked('prj_api', true), linked('prj_web', false)]
    render(<LearnAsks />)
    expect(await screen.findByText('Turn on learning for api?')).toBeTruthy()
    expect(screen.queryByText('Turn on learning for web?')).toBeNull()
    expect(answered).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Turn on' }))
    await waitFor(() => expect(answered).toHaveBeenCalledWith('prj_api', true))
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull())
  })

  it('leaves the project off on Not now, and asks again only when the app says a project waits', async () => {
    projects = [linked('prj_api', true)]
    render(<LearnAsks />)
    await screen.findByText('Turn on learning for api?')
    fireEvent.click(screen.getByRole('button', { name: 'Not now' }))
    await waitFor(() => expect(answered).toHaveBeenCalledWith('prj_api', false))
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull())
    projects = [...projects, linked('prj_web', true)]
    await act(async () => heard['learning:asked']?.(null))
    expect(await screen.findByText('Turn on learning for web?')).toBeTruthy()
    expect(screen.queryByText('Turn on learning for api?')).toBeNull()
  })

  it('asks nothing while Labs has the project agent off', async () => {
    labs = false
    projects = [linked('prj_api', true)]
    render(<LearnAsks />)
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull())
  })
})
