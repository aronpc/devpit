import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProjectMemory } from '../gen/bindings'
import { MemoryPane } from './MemoryPane'
import { ProjectPlaces } from './ProjectPlaces'

const called = vi.fn()
const shown = vi.fn()

const memory = (): ProjectMemory => ({
  projectId: 'prj_api',
  kept: true,
  facts: [{ day: '2026-11-14', source: 'AGENTS.md', text: 'Everything versioned is in English.', files: ['AGENTS.md'], cards: [] }],
  lessons: [{ day: '2026-11-13', source: '#12', text: 'Wait for the queue before asserting.', files: [], cards: ['#12'] }],
  decisions: [{ day: '2026-11-12', source: 'docs/adr/0001-sync.md', text: 'Sync the workspace, not the machine.', files: ['docs/adr/0001-sync.md'], cards: [] }],
  proposals: [
    {
      proposal: { projectId: 'prj_api', project: 'api', day: '2026-11-14', source: 'onboarding, Makefile', file: 'facts.md', text: 'make test runs every test.', replaces: null, onboarding: true },
      files: ['Makefile'],
      cards: [],
    },
    {
      proposal: { projectId: 'prj_api', project: 'api', day: '2026-11-14', source: '#13', file: 'lessons.md', text: 'Retry a busy script.', replaces: null, onboarding: false },
      files: [],
      cards: ['#13'],
    },
  ],
  learning: false,
  learned: '2026-11-14',
})

vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: new Proxy(
    {},
    {
      get: (_, name) =>
        typeof name !== 'string' || name === 'then'
          ? undefined
          : async (...args: unknown[]) => {
              called(name, ...args)
              if (name === 'maintenanceRead') return { projectAgent: true }
              if (name === 'projectAgentRelearn') return { ...memory(), learning: true }
              if (name === 'lessonDecide') return { proposals: [], decisions: [] }
              return memory()
            },
    },
  ),
}))
vi.mock('./useShell', () => ({ useShell: () => ({ project: { id: 'prj_api', name: 'api', orchestrator: null }, prefs: null }) }))
vi.mock('./shellStore', () => ({ useShellPick: () => shown }))
vi.mock('./window', () => ({ onCarried: () => () => undefined }))

afterEach(() => {
  cleanup()
  called.mockReset()
  shown.mockReset()
})

describe('The project agent page', () => {
  it('is a place of a project, beside Ask the project', async () => {
    const open = vi.fn()
    render(<ProjectPlaces open={open} showing={() => false} />)
    await waitFor(() => expect(called).toHaveBeenCalledWith('maintenanceRead'))
    fireEvent.click(screen.getByRole('button', { name: 'Project agent' }))
    expect(open).toHaveBeenCalledWith('memory')
  })

  it('lists facts, lessons, decisions and proposals with their sources', async () => {
    render(<MemoryPane />)
    await screen.findByText('Everything versioned is in English.')
    for (const [section, text] of [
      ['Facts', 'Everything versioned is in English.'],
      ['Lessons', 'Wait for the queue before asserting.'],
      ['Decisions', 'Sync the workspace, not the machine.'],
      ['Proposed', 'make test runs every test.'],
    ]) {
      expect(within(screen.getByRole('region', { name: section })).getByText(text)).toBeTruthy()
    }
    expect(within(screen.getByRole('region', { name: 'Lessons' })).getByText(/#12/)).toBeTruthy()
    fireEvent.click(within(screen.getByRole('region', { name: 'Decisions' })).getByRole('button', { name: 'docs/adr/0001-sync.md' }))
    expect(shown).toHaveBeenCalledWith('file', { id: 'file:docs/adr/0001-sync.md', path: 'docs/adr/0001-sync.md' })
  })

  it('approves, edits and discards a proposal from the page', async () => {
    render(<MemoryPane />)
    const proposed = await screen.findByRole('region', { name: 'Proposed' })
    const first = within(proposed).getAllByRole('group')[0]!
    fireEvent.click(within(first).getByRole('button', { name: 'Approve' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('lessonDecide', 'prj_api', 'onboarding, Makefile', 'make test runs every test.', null, 'make test runs every test.'))

    const second = within(await screen.findByRole('region', { name: 'Proposed' })).getAllByRole('group')[1]!
    fireEvent.click(within(second).getByRole('button', { name: 'Edit' }))
    fireEvent.change(within(second).getByRole('textbox', { name: 'Proposal' }), { target: { value: 'Retry a script that is busy.' } })
    fireEvent.click(within(second).getByRole('button', { name: 'Approve' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('lessonDecide', 'prj_api', '#13', 'Retry a busy script.', null, 'Retry a script that is busy.'))

    const again = within(await screen.findByRole('region', { name: 'Proposed' })).getAllByRole('group')[1]!
    fireEvent.click(within(again).getByRole('button', { name: 'Discard' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('lessonDecide', 'prj_api', '#13', 'Retry a busy script.', null, null))
  })

  it('approves every proposal of its own read at once, and learns the project again', async () => {
    render(<MemoryPane />)
    fireEvent.click(await screen.findByRole('button', { name: 'Approve all from onboarding (1)' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('projectAgentApproveLearned', 'prj_api'))
    fireEvent.click(screen.getByRole('button', { name: 'Re-learn' }))
    await screen.findByText(/Learning the project/)
    expect(called).toHaveBeenCalledWith('projectAgentRelearn', 'prj_api')
  })
})
