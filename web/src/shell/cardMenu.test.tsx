import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Card, CardDetail, Column, DeleteRefusal } from '../gen/bindings'
import { setLocale } from '../i18n'
import type { Lane } from './board'
import { cardMenu } from './cardMenu'
import { wired } from './fileMenu'
import { LaneCards } from './LaneCards'
import { copyBranch, type CardBoardActs } from './useCardActs'
import type { Drag } from './useDrag'

afterEach(cleanup)

const detail = { worktree: { branch: 'devpit/wire-the-board', path: '/w', baseRef: null, dirtyFiles: 0, exists: true } }

vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: { cardDetail: () => detail as unknown as CardDetail },
}))
vi.mock('./useShell', () => ({ useShell: () => ({ show: vi.fn() }) }))

const card = (over: Partial<Card> = {}): Card => ({
  id: 'card_1',
  columnId: 'col_1',
  title: 'Wire the board',
  body: '',
  position: 0,
  worktreePath: null,
  dueAt: null,
  dueTime: false,
  costUsd: null,
  comments: 2,
  pinned: 1,
  runs: [],
  activity: null,
  ...over,
})

const column = (over: Partial<Column> = {}): Column => ({
  id: 'col_1',
  name: 'Todo',
  position: 0,
  step: null,
  onPass: null,
  autonomy: 'manual',
  role: null,
  roleChosen: false,
  ...over,
})

const drag: Drag = { held: null, landing: null, landed: null, down: vi.fn(), move: vi.fn(), up: vi.fn(() => ({ what: 'nothing' as const })) }

const hands = { open: vi.fn(), rename: vi.fn(), terminal: vi.fn(), chat: vi.fn(), archive: vi.fn(), remove: vi.fn() }

describe('the card menu', () => {
  it('has something behind every entry', () => {
    expect(wired(cardMenu(hands))).toBe(true)
    expect(wired(cardMenu({ ...hands, play: vi.fn(), copyBranch: vi.fn() }))).toBe(true)
  })

  it('would catch an entry that only looks like a control', () => {
    expect(wired([...cardMenu(hands), { label: 'Open in a chat' }])).toBe(false)
  })

  it('runs the step where the lane has one, and opens a terminal where it does not', () => {
    const labels = (menu: ReturnType<typeof cardMenu>) => menu.filter((one) => !one.rule).map((one) => one.label)
    expect(labels(cardMenu(hands))).toEqual(['Open', 'Rename', 'Open a terminal', 'Chat about this card', 'Archive', 'Delete…'])
    expect(labels(cardMenu({ ...hands, play: vi.fn(), copyBranch: vi.fn() }))).toEqual([
      'Open',
      'Rename',
      'Run step',
      'Chat about this card',
      'Copy branch',
      'Archive',
      'Delete…',
    ])
  })
})

describe('the card menu in Portuguese', () => {
  afterEach(() => setLocale('en'))

  it('says every entry in the chosen language', () => {
    setLocale('pt-BR')
    const all = cardMenu({ ...hands, moveTo: vi.fn(), play: vi.fn(), copyBranch: vi.fn() })
    expect(all.filter((one) => !one.rule).map((one) => one.label)).toEqual([
      'Abrir',
      'Renomear',
      'Mover para…',
      'Rodar o passo',
      'Conversar sobre este cartão',
      'Copiar a branch',
      'Arquivar',
      'Excluir…',
    ])
    expect(cardMenu(hands).find((one) => one.run === hands.terminal)?.label).toBe('Abrir um terminal')
  })
})

describe('a card on the board', () => {
  const acts = (over: Partial<CardBoardActs> = {}): CardBoardActs => ({
    terminal: vi.fn(),
    chat: vi.fn(),
    archive: vi.fn(() => Promise.resolve(null)),
    remove: vi.fn(() => Promise.resolve<DeleteRefusal | string | null>(null)),
    liveWork: vi.fn(() => Promise.resolve({ tabs: [], runs: [] })),
    stopLive: vi.fn(() => Promise.resolve(null)),
    archived: vi.fn(),
    problem: vi.fn(),
    ...over,
  })

  const board = (
    props: {
      cards?: Card[]
      acts?: CardBoardActs
      onRename?: () => void
      onOpen?: () => void
      others?: { id: string; name: string }[]
      onMove?: (cardId: string, columnId: string) => void
    } = {},
  ) => {
    const lane = { column: column(), cards: props.cards ?? [card()] } as Lane
    return render(
      <LaneCards
        lane={lane}
        drag={drag}
        dropping={false}
        progress={{}}
        onOpen={props.onOpen ?? vi.fn()}
        onPlay={vi.fn(() => Promise.resolve(null))}
        onRename={props.onRename ?? vi.fn()}
        others={props.others ?? []}
        onMove={props.onMove ?? vi.fn()}
        acts={() => props.acts ?? acts()}
      />,
    )
  }

  const rightClick = (container: HTMLElement): void => {
    fireEvent.contextMenu(container.querySelector('[data-id="card_1"]')!, { clientX: 40, clientY: 40 })
  }

  it('publishes its id where a menu reads it, and opens its menu on a right-click', () => {
    const { container } = board()
    rightClick(container)
    const items = within(screen.getByRole('menu')).getAllByRole('menuitem').map((item) => item.getAttribute('aria-label'))
    expect(items).toEqual(['Open', 'Rename', 'Open a terminal', 'Chat about this card', 'Archive', 'Delete…'])
  })

  it('starts a chat about the card from its menu', () => {
    const hands = acts()
    const { container } = board({ acts: hands })
    rightClick(container)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Chat about this card' }))
    expect(hands.chat).toHaveBeenCalled()
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('offers its branch only once it has a checkout', () => {
    const { container } = board({ cards: [card({ worktreePath: '/w/card_1' })], acts: acts({ copyBranch: vi.fn() }) })
    rightClick(container)
    expect(screen.getByRole('menuitem', { name: 'Copy branch' })).toBeTruthy()
  })

  it('renames in place', () => {
    const onRename = vi.fn()
    const { container } = board({ onRename })
    rightClick(container)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Rename' }))
    const field = screen.getByRole('textbox', { name: 'Card title' })
    fireEvent.change(field, { target: { value: 'Wire the whole board' } })
    fireEvent.keyDown(field, { key: 'Enter' })
    expect(onRename).toHaveBeenCalledWith('card_1', 'Wire the whole board')
  })

  it('opens the card, and deletes it only after saying what goes', async () => {
    const onOpen = vi.fn()
    const remove = vi.fn(() => Promise.resolve<DeleteRefusal | string | null>(null))
    const { container } = board({ onOpen, acts: acts({ remove }) })
    rightClick(container)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Open' }))
    expect(onOpen).toHaveBeenCalledWith('card_1')

    rightClick(container)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Delete…' }))
    expect(await screen.findByText(/Its 2 comments, 1 pinned file and 0 runs go with it/)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }))
    await waitFor(() => expect(remove).toHaveBeenCalledWith(false))
  })

  it('does not ask to archive until it knows what is still going on the card', async () => {
    let read = (_live: { tabs: string[]; runs: string[] }): void => undefined
    const liveWork = vi.fn(() => new Promise<{ tabs: string[]; runs: string[] }>((resolve) => (read = resolve)))
    const { container } = board({ acts: acts({ liveWork }) })
    rightClick(container)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Archive' }))
    expect(screen.queryByRole('dialog')).toBeNull()
    read({ tabs: ['tab_1'], runs: [] })
    expect(await screen.findByRole('button', { name: 'Stop it and archive' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Archive' })).toBeNull()
  })

  describe('moving a card from its menu', () => {
  it('offers the other lanes and moves the card to the one picked', () => {
    const onMove = vi.fn()
    const { container } = board({ others: [{ id: 'col_2', name: 'Doing' }, { id: 'col_3', name: 'Done' }], onMove })
    rightClick(container)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Move to…' }))
    const lanes = within(screen.getByRole('menu')).getAllByRole('menuitem').map((item) => item.textContent)
    expect(lanes).toEqual(['Doing', 'Done'])
    fireEvent.click(screen.getByRole('menuitem', { name: 'Done' }))
    expect(onMove).toHaveBeenCalledWith('card_1', 'col_3')
  })

  it('does not offer Move to… on a board with one lane', () => {
    const { container } = board()
    rightClick(container)
    expect(screen.queryByRole('menuitem', { name: 'Move to…' })).toBeNull()
  })
  })
})

describe('copying a branch', () => {
  it('puts the branch of the card on the clipboard', async () => {
    const writeText = vi.fn(() => Promise.resolve())
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    expect(await copyBranch('p1', 'card_1')).toBeNull()
    expect(writeText).toHaveBeenCalledWith('devpit/wire-the-board')
  })

  it('says so when the clipboard refuses, rather than throwing it away', async () => {
    const writeText = vi.fn(() => Promise.reject(new Error('NotAllowedError')))
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    expect(await copyBranch('p1', 'card_1')).toMatch(/devpit\/wire-the-board/)
  })
})
