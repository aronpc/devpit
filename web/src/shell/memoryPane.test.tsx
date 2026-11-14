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
  facts: [
    { day: '2026-11-14', source: 'AGENTS.md', text: 'Everything versioned is in English.', files: ['AGENTS.md'], cards: [], auto: false },
    { day: '2026-11-14', source: 'Makefile, accepted automatically', text: 'make fmt formats Rust and the web.', files: ['Makefile'], cards: [], auto: true },
  ],
  lessons: [{ day: '2026-11-13', source: '#12', text: 'Wait for the queue before asserting.', files: [], cards: ['#12'], auto: false }],
  decisions: [{ day: '2026-11-12', source: 'docs/adr/0001-sync.md', text: 'Sync the workspace, not the machine.', files: ['docs/adr/0001-sync.md'], cards: [], auto: false }],
  proposals: [
    {
      proposal: { id: 'a1', by: 'onboarding', summary: 'make test runs every test.', projectId: 'prj_api', project: 'api', day: '2026-11-14', source: 'onboarding, Makefile', file: 'facts.md', text: 'make test runs every test.', replaces: null, onboarding: true },
      files: ['Makefile'],
      cards: [],
    },
    {
      proposal: {
        id: 'b2',
        by: 'a session on card #13',
        summary: 'Scripts can be busy: try again.',
        projectId: 'prj_api',
        project: 'api',
        day: '2026-11-14',
        source: '#13',
        file: 'lessons.md',
        text: 'Retry a busy script. A script holding the lock answers EBUSY for up to 3 s.',
        replaces: null,
        onboarding: false,
      },
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
              if (name === 'lessonsProposed') return { proposals: memory().proposals.map((one) => one.proposal), decisions: [] }
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
    fireEvent.click(screen.getByRole('button', { name: /^Project agent/ }))
    expect(open).toHaveBeenCalledWith('memory')
  })

  it('counts what waits for the person beside it, never in a banner', async () => {
    render(<ProjectPlaces open={vi.fn()} showing={() => false} />)
    expect(await screen.findByRole('button', { name: 'Project agent, 2 to review' })).toBeTruthy()
  })

  it('shows each proposal as a card: its kind, who proposed it, a plain line and its sources', async () => {
    render(<MemoryPane />)
    const card = await screen.findByRole('group', { name: 'Scripts can be busy: try again.' })
    expect(within(card).getByText('Lesson')).toBeTruthy()
    expect(within(card).getByText('from a session on card #13')).toBeTruthy()
    expect(within(card).getByText('Sources')).toBeTruthy()
    expect(within(card).getByText('#13')).toBeTruthy()
    const details = within(card).getByText('details').closest('details')!
    expect(details.open).toBe(false)
    expect(within(details).getByText('Retry a busy script. A script holding the lock answers EBUSY for up to 3 s.')).toBeTruthy()
    const learned = screen.getByRole('group', { name: 'make test runs every test.' })
    expect(within(learned).getByText('Fact')).toBeTruthy()
    expect(within(learned).getByText('from onboarding')).toBeTruthy()
    expect(within(learned).getByRole('button', { name: 'Makefile' })).toBeTruthy()
    // A plain line that is the whole text has nothing behind "details".
    expect(within(learned).queryByText('details')).toBeNull()
  })

  it('says which fact was accepted automatically and undoes it', async () => {
    render(<MemoryPane />)
    const facts = await screen.findByRole('region', { name: 'Facts' })
    expect(within(facts).getAllByText('accepted automatically')).toHaveLength(1)
    fireEvent.click(within(facts).getByRole('button', { name: 'Undo' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('projectAgentUndoAccepted', 'prj_api', 'make fmt formats Rust and the web.'))
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
    await waitFor(() =>
      expect(called).toHaveBeenCalledWith('lessonDecide', 'prj_api', '#13', 'Retry a busy script. A script holding the lock answers EBUSY for up to 3 s.', null, 'Retry a script that is busy.'),
    )

    const again = within(await screen.findByRole('region', { name: 'Proposed' })).getAllByRole('group')[1]!
    fireEvent.click(within(again).getByRole('button', { name: 'Discard' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('lessonDecide', 'prj_api', '#13', 'Retry a busy script. A script holding the lock answers EBUSY for up to 3 s.', null, null))
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
