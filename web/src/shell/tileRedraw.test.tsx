import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Board, Card, CardHappening, Run } from '../gen/bindings'
import { BoardPane } from './BoardPane'
import { nearness } from './due'

/*
 * An agent's turn says what a card is doing many times over, and a running
 * step prints line after line. Each one drew every tile on the board again.
 *
 * A tile reads its card's due date once per draw, so the reads of it count
 * the tiles drawn.
 */

/* Set by a test, for the read after the first: card b, renamed somewhere else. */
let renamed = false

afterEach(() => {
  cleanup()
  renamed = false
})

const handlers: Record<string, (payload: unknown) => void> = {}

const run: Run = {
  id: 'run_c',
  stepId: 'step_1',
  stepName: 'tests',
  state: 'running',
  output: null,
  exitCode: null,
  costUsd: null,
  durationMs: null,
  profile: null,
  model: null,
  startedAt: 0,
}

const card = (id: string, position: number, over: Partial<Card> = {}): Card => ({
  id,
  columnId: 'col_1',
  title: `Card ${id}`,
  body: '',
  position,
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
  columns: [{ id: 'col_1', name: 'Todo', position: 0, step: null, onPass: null, autonomy: 'manual', role: null, roleChosen: false }],
  cards: [card('a', 0), card('b', 1, renamed ? { title: 'Card b, renamed' } : {}), card('c', 2, { runs: [run] })],
  steps: [],
})

vi.mock('./due', async (actual) => {
  const real = await actual<typeof import('./due')>()
  return { ...real, nearness: vi.fn(real.nearness) }
})
vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: { boardGet: () => board(), boardArchived: () => ({ cards: [] }) },
}))
vi.mock('./window', () => ({
  onCarried: (name: string, then: (payload: unknown) => void) => {
    handlers[name] = then
    return () => undefined
  },
}))
vi.mock('./useShell', () => ({
  useShell: () => ({ project: { id: 'p1' }, active: { kind: 'board' }, wantedCard: null, openCard: vi.fn(), show: vi.fn() }),
}))

const drawn = vi.mocked(nearness)

describe('a tile on the board', () => {
  it("is not drawn again for another card's activity", async () => {
    render(<BoardPane />)
    await screen.findByText('Card c')
    drawn.mockClear()

    const working: CardHappening = { cardId: 'a', activity: 'working', sessions: [] }
    act(() => handlers['card:happening']!(working))

    /* Awaited: on a loaded machine the tile can paint a frame after the act. */
    expect(await screen.findAllByRole('button', { name: /working/i })).toHaveLength(1)
    expect(drawn).toHaveBeenCalledTimes(1)
  })

  it("is not drawn again for a line another card's step printed", async () => {
    render(<BoardPane />)
    await screen.findByText('Card c')
    drawn.mockClear()

    act(() => handlers['run:progress']!(['run_c', 'compiling 3 of 9']))

    expect(screen.getByText('compiling 3 of 9')).toBeTruthy()
    expect(drawn).toHaveBeenCalledTimes(1)
  })

  /* Every read answers new objects for every card, and an agent's turn is
     many reads: each one drew the whole board again. */
  it('is drawn again on a reload only when its own card changed', async () => {
    render(<BoardPane />)
    await screen.findByText('Card c')
    drawn.mockClear()

    renamed = true
    act(() => handlers['board:changed']!('p1'))

    expect(await screen.findByText('Card b, renamed')).toBeTruthy()
    expect(drawn).toHaveBeenCalledTimes(1)
  })
})
