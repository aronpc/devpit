import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Board, Card, Played, Run } from '../gen/bindings'
import { BoardPane } from './BoardPane'
import { Tile } from './Lane'

afterEach(cleanup)

const run: Run = {
  id: 'run_1',
  stepId: 'step_1',
  stepName: 'deploy',
  state: 'running',
  output: null,
  exitCode: null,
  costUsd: null,
  durationMs: null,
  profile: null,
  model: null,
  startedAt: 0,
}

const card = (over: Partial<Card> = {}): Card => ({
  id: 'card_1',
  columnId: 'col_1',
  title: 'Ship it',
  body: '',
  position: 0,
  worktreePath: null,
  dueAt: null,
  dueTime: false,
  costUsd: null,
  comments: 0,
  pinned: 0,
  runs: [],
  activity: null,
  ...over,
})

const board = (): Board => ({
  projectId: 'p1',
  columns: [
    { id: 'col_1', name: 'Todo', position: 0, step: null, onPass: null, autonomy: 'manual', role: null, roleChosen: false },
    {
      id: 'col_2',
      name: 'Test',
      position: 1,
      step: { id: 'step_1', kind: 'command', name: 'tests', config: '{}', irreversible: false },
      onPass: null,
      autonomy: 'manual',
      role: null,
      roleChosen: false,
    },
  ],
  cards: [card(), card({ id: 'card_2', columnId: 'col_2', title: 'Check it' })],
  steps: [],
})

vi.mock('./live', () => ({
  ask: async (call: () => unknown) => {
    try {
      return { data: await call(), error: null, loading: false }
    } catch (thrown) {
      return { data: null, error: (thrown as Error).message, loading: false }
    }
  },
  commands: {
    boardGet: () => board(),
    cardPlay: () => {
      throw new Error('something is already running on this card')
    },
  },
}))
vi.mock('./window', () => ({ onCarried: () => () => undefined }))
vi.mock('./useShell', () => ({
  useShell: () => ({ project: { id: 'p1' }, wantedCard: null, openCard: vi.fn() }),
}))

describe('the play button on a tile', () => {
  it('runs the step, and asks first when the step has no undo', async () => {
    const said: Played[] = [
      { run: null, needsConfirming: true, laneRunsNothing: false },
      { run, needsConfirming: false, laneRunsNothing: false },
    ]
    const onPlay = vi.fn((_cardId: string, _confirmed: boolean) => Promise.resolve<Played | null>(said.shift() ?? null))
    render(<Tile card={card()} stepName="deploy" onPlay={onPlay} />)

    fireEvent.click(screen.getByRole('button', { name: 'Run deploy' }))
    expect(onPlay).toHaveBeenCalledWith('card_1', false)
    const question = await screen.findByRole('dialog')
    expect(within(question).getByText('Run deploy?')).toBeTruthy()

    fireEvent.click(within(question).getByRole('button', { name: 'Run deploy' }))
    await waitFor(() => expect(onPlay).toHaveBeenLastCalledWith('card_1', true))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  })

  it('is only drawn on tiles in a lane that runs a step, and says why a run was refused', async () => {
    render(<BoardPane />)
    await screen.findByText('Check it')
    const plays = screen.getAllByRole('button', { name: /^Run / })
    expect(plays).toHaveLength(1)

    fireEvent.click(plays[0]!)
    expect(await screen.findByRole('button', { name: 'something is already running on this card' })).toBeTruthy()
  })
})
