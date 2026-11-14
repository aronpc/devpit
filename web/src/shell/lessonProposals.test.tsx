import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { LessonProposal, MemoryDecision, LessonProposals as Proposals } from '../gen/bindings'
import { LessonProposals } from './LessonProposals'

vi.mock('./window', () => ({
  onCarried: () => () => undefined,
}))

const proposed = (source: string, text: string, replaces: string | null = null): LessonProposal => ({
  id: `${source} ${text}`,
  by: `a session on card ${source}`,
  summary: text,
  projectId: 'prj_api',
  project: 'asc-api',
  day: '2026-11-13',
  source,
  file: replaces === null ? 'lessons.md' : 'facts.md',
  text,
  replaces,
  onboarding: false,
})

let waiting: LessonProposal[] = []
let decisions: MemoryDecision[] = []
const decided = vi.fn()
vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => {
    try {
      return { data: await call(), error: null, loading: false }
    } catch (why) {
      return { data: null, error: String(why), loading: false }
    }
  },
  commands: {
    lessonsProposed: async (): Promise<Proposals> => ({ proposals: waiting, decisions }),
    lessonDecide: async (projectId: string, source: string, text: string, replaces: string | null, kept: string | null): Promise<Proposals> => {
      decided(projectId, source, text, replaces, kept)
      waiting = waiting.filter((one) => one.text !== text)
      return { proposals: waiting, decisions }
    },
  },
}))

afterEach(() => {
  cleanup()
  waiting = []
  decisions = []
  decided.mockReset()
})

describe('LessonProposals', () => {
  it('draws nothing while no lesson waits', async () => {
    render(<LessonProposals projectId="prj_api" source="#331" />)
    await waitFor(() => expect(screen.queryByRole('region')).toBeNull())
  })

  it('approves a lesson as proposed, and it goes', async () => {
    waiting = [proposed('#331', 'Wait 3 s for the queue test.')]
    render(<LessonProposals projectId="prj_api" source="#331" />)
    fireEvent.click(await screen.findByRole('button', { name: 'Approve' }))
    await waitFor(() => expect(decided).toHaveBeenCalledWith('prj_api', '#331', 'Wait 3 s for the queue test.', null, 'Wait 3 s for the queue test.'))
    await waitFor(() => expect(screen.queryByText('Wait 3 s for the queue test.')).toBeNull())
  })

  it('approves a lesson as edited', async () => {
    waiting = [proposed('#331', 'Wait 3 s for the queue test.')]
    render(<LessonProposals projectId="prj_api" source="#331" />)
    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }))
    fireEvent.change(screen.getByRole('textbox', { name: 'Lesson' }), { target: { value: 'Wait 3 s under load.' } })
    fireEvent.click(screen.getByRole('button', { name: 'Approve' }))
    await waitFor(() => expect(decided).toHaveBeenCalledWith('prj_api', '#331', 'Wait 3 s for the queue test.', null, 'Wait 3 s under load.'))
  })

  it('discards a lesson, keeping nothing', async () => {
    waiting = [proposed('#331', 'Run make fmt.')]
    render(<LessonProposals projectId="prj_api" source="#331" />)
    fireEvent.click(await screen.findByRole('button', { name: 'Discard' }))
    await waitFor(() => expect(decided).toHaveBeenCalledWith('prj_api', '#331', 'Run make fmt.', null, null))
  })

  it("shows a card only the lessons it proposed", async () => {
    waiting = [proposed('#331', 'Mine.'), proposed('#12', 'Not mine.')]
    render(<LessonProposals projectId="prj_api" source="#331" />)
    expect(await screen.findByText('Mine.')).toBeTruthy()
    expect(screen.queryByText('Not mine.')).toBeNull()
  })

  it('shows a stale fact as a proposal with what it replaces, changed only once approved', async () => {
    waiting = [proposed('AGENTS.md', 'Read "Working on devpit".', 'Read "Working in this repository".')]
    render(<LessonProposals projectId="prj_api" source="AGENTS.md" />)
    expect(await screen.findByText('Read "Working on devpit".')).toBeTruthy()
    expect(screen.getByText(/was: Read "Working in this repository"\./)).toBeTruthy()
    expect(decided).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Approve' }))
    await waitFor(() =>
      expect(decided).toHaveBeenCalledWith('prj_api', 'AGENTS.md', 'Read "Working on devpit".', 'Read "Working in this repository".', 'Read "Working on devpit".'),
    )
  })

  it('proposes removing a stale fact', async () => {
    waiting = [proposed('README.md', '', 'Builds with make.')]
    render(<LessonProposals projectId="prj_api" source="README.md" />)
    expect(await screen.findByText('Remove: Builds with make.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Approve' }))
    await waitFor(() => expect(decided).toHaveBeenCalledWith('prj_api', 'README.md', '', 'Builds with make.', ''))
  })

  it('shows a card the decisions it produced, each with its cards', async () => {
    decisions = [
      { projectId: 'prj_api', day: '2026-11-14', cards: ['#331', '#332'], source: '#331, #332, docs/adr/0002.md', text: 'Proposals share one queue.' },
      { projectId: 'prj_api', day: '2026-11-14', cards: ['#12'], source: '#12', text: 'Not this card.' },
    ]
    render(<LessonProposals projectId="prj_api" source="#332" />)
    const made = await screen.findByRole('region', { name: 'Decisions from this card' })
    expect(made.textContent).toContain('Proposals share one queue.')
    expect(made.textContent).toContain('#331, #332')
    expect(made.textContent).not.toContain('Not this card.')
  })
})
