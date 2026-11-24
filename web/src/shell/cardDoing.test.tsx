import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Board, Card, CardHappening } from '../gen/bindings'
import { setLocale } from '../i18n'
import { doingLabel, useCardUnread, useVisit, waitingSession } from './cardDoing'
import { Tile } from './Lane'
import { useBoard } from './useBoard'

afterEach(cleanup)

const handlers: Record<string, (payload: unknown) => void> = {}
const boardGets = vi.fn()
const focused = vi.fn()
const shown = vi.fn()

const card = (id: string, over: Partial<Card> = {}): Card => ({
  id,
  columnId: 'col_1',
  title: `Card ${id}`,
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
  columns: [{ id: 'col_1', name: 'Todo', position: 0, step: null, onPass: null, autonomy: 'manual', role: null, roleChosen: false }],
  cards: [card('a'), card('b', { position: 1 })],
  steps: [],
})

vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: {
    boardGet: () => {
      boardGets()
      return board()
    },
    sessionFocus: (project: string, tab: string, leaf: string) => {
      focused(project, tab, leaf)
      return null
    },
  },
}))
vi.mock('./window', () => ({
  onCarried: (name: string, then: (payload: unknown) => void) => {
    handlers[name] = then
    return () => undefined
  },
}))
vi.mock('./useShell', () => ({ useShell: () => ({ show: shown }) }))

const waiting: CardHappening = {
  cardId: 'a',
  activity: 'waiting',
  sessions: [{ kind: 'pane', ref: 'leaf_1', state: 'waiting', tabId: 'tab_of_a', leafId: 'leaf_1', runId: null, step: null }],
}

describe("a card's happening on the board", () => {
  it('changes that card and no other, without reading the board again', async () => {
    const { result } = renderHook(() => useBoard('p1'))
    await waitFor(() => expect(result.current.lanes[0]?.cards).toHaveLength(2))
    act(() => handlers['card:happening']!(waiting))

    const cards = result.current.lanes[0]!.cards
    expect(cards.find((one) => one.id === 'a')?.activity).toBe('waiting')
    expect(cards.find((one) => one.id === 'b')?.activity).toBeNull()
    expect(result.current.sessions.a).toHaveLength(1)
    expect(boardGets).toHaveBeenCalledTimes(1)
  })
})

describe("a tile's dot", () => {
  it('says what the sessions are doing, and that only the open project is followed while it waits for word', () => {
    expect(doingLabel('waiting', false)).toBe('An agent is waiting on you')
    expect(doingLabel('open', false)).toContain('shows once it reports')
    expect(doingLabel('open', false)).toContain('Only the project open in this window is followed')
    expect(doingLabel('done', true)).toContain('nobody has looked')
    expect(doingLabel('failed', false)).toBe('A run on this card failed')
  })

  it('is drawn only when something is working on the card, and goes where it points', () => {
    const onDoing = vi.fn()
    const { rerender } = render(<Tile card={card('a', { activity: 'waiting' })} onDoing={onDoing} />)
    fireEvent.click(screen.getByRole('button', { name: 'An agent is waiting on you' }))
    expect(onDoing).toHaveBeenCalled()

    rerender(<Tile card={card('a')} onDoing={onDoing} />)
    expect(screen.queryByRole('button')).toBeNull()
  })

  it('takes somebody to the pane waiting on them, and otherwise opens the card', () => {
    expect(waitingSession(waiting.sessions)?.leafId).toBe('leaf_1')
    const onOpen = vi.fn()
    const { result } = renderHook(() => useVisit('p1', { a: waiting.sessions }, onOpen))

    result.current(card('a'))
    expect(shown).toHaveBeenCalledWith('term', { id: 'tab_of_a', title: 'Card a', cardId: 'a' })
    expect(focused).toHaveBeenCalledWith('p1', 'tab_of_a', 'leaf_1')

    result.current(card('b'))
    expect(onOpen).toHaveBeenCalledWith('b')
  })
})

describe("a tile's dot in Portuguese", () => {
  afterEach(() => setLocale('en'))

  it('says what the sessions are doing in the chosen language', () => {
    setLocale('pt-BR')
    expect(doingLabel('working', false)).toBe('Um agente está trabalhando neste cartão')
    expect(doingLabel('waiting', false)).toBe('Um agente está esperando você')
    expect(doingLabel('done', true)).toBe('Um agente terminou, e ninguém olhou ainda')
    expect(doingLabel('gone', false)).toBe('A sessão deste cartão terminou')
  })
})

describe('a finish nobody has looked at', () => {
  it('is marked when a card finishes out of sight, and cleared when the card is opened', () => {
    const { result, rerender } = renderHook(({ cards, looking }) => useCardUnread(cards, looking), {
      initialProps: { cards: [card('a', { activity: 'done' })], looking: null as string | null },
    })
    // Seen for the first time already finished: nothing to catch up on.
    expect(result.current.has('a')).toBe(false)

    rerender({ cards: [card('a', { activity: 'working' })], looking: null })
    rerender({ cards: [card('a', { activity: 'done' })], looking: null })
    expect(result.current.has('a')).toBe(true)

    rerender({ cards: [card('a', { activity: 'done' })], looking: 'a' })
    expect(result.current.has('a')).toBe(false)
  })
})
