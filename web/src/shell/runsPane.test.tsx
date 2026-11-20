import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProjectRun, RunsPage, RunsQuery } from '../gen/bindings'
import type { Lane } from './board'
import { RunsPane } from './RunsPane'

afterEach(cleanup)

const asked: RunsQuery[] = []

const listed = (id: string, title: string, state: ProjectRun['run']['state']): ProjectRun => ({
  run: {
    id,
    stepId: 'step_1',
    stepName: 'tests',
    state,
    output: null,
    exitCode: null,
    costUsd: null,
    durationMs: null,
    profile: null,
    model: null,
    startedAt: 100,
  },
  cardId: `card_${id}`,
  cardTitle: title,
})

vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve({ data: call(), error: null, loading: false }),
  commands: {
    runsList: (query: RunsQuery): RunsPage => {
      asked.push(query)
      if (query.after) return { runs: [listed('r3', 'Third card', 'ok')], next: null }
      if (query.state === 'lost') return { runs: [listed('r9', 'Lost card', 'lost')], next: null }
      return {
        runs: [listed('r1', 'First card', 'failed'), listed('r2', 'Second card', 'ok')],
        next: { startedAt: 100, id: 'r2' },
      }
    },
  },
}))

const lanes = [
  {
    column: {
      id: 'col_1',
      name: 'Review',
      position: 0,
      step: { id: 'step_1', kind: 'command', name: 'tests', config: '{}', irreversible: false },
      onPass: null,
    },
    cards: [],
  },
] as unknown as Lane[]

const open = (onOpenCard = vi.fn()) =>
  render(<RunsPane projectId="prj" lanes={lanes} onClose={vi.fn()} onOpenCard={onOpenCard} />)

describe('the runs across the board', () => {
  it('pages on from where the last page ended', async () => {
    asked.length = 0
    open()
    await screen.findByText('First card')
    fireEvent.click(screen.getByText('Load more'))
    await screen.findByText('Third card')
    expect(asked.at(-1)?.after).toEqual({ startedAt: 100, id: 'r2' })
    // The earlier page stays, and the last page offers nothing more.
    expect(screen.getByText('First card')).toBeTruthy()
    expect(screen.queryByText('Load more')).toBeNull()
  })

  it('starts over from the top when a filter changes', async () => {
    asked.length = 0
    open()
    await screen.findByText('First card')
    fireEvent.change(screen.getByLabelText('State'), { target: { value: 'lost' } })
    await screen.findByText('Lost card')
    expect(asked.at(-1)).toMatchObject({ state: 'lost', after: null })
    expect(screen.queryByText('First card')).toBeNull()
  })

  it('offers the lanes that run something, by the step they run', async () => {
    asked.length = 0
    open()
    fireEvent.change(screen.getByLabelText('Lane'), { target: { value: 'step_1' } })
    await waitFor(() => expect(asked.at(-1)?.stepId).toBe('step_1'))
  })

  it('opens the card a run belongs to', async () => {
    const onOpenCard = vi.fn()
    open(onOpenCard)
    fireEvent.click(await screen.findByText('First card'))
    expect(onOpenCard).toHaveBeenCalledWith('card_r1')
  })
})
