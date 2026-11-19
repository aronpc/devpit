import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { IslandChange, IslandQuestion, IslandSession, Question } from '../gen/bindings'
import { setLocale } from '../i18n'
import { IslandWindow } from './IslandWindow'

/* What the window would hear, by event name, so a test can say it. */
const heard = new Map<string, (payload: unknown) => void>()
/* What Rust holds, as the island asks for it again: the sessions said. */
const alive = new Map<string, IslandSession>()
const say = (name: string, payload: unknown): void => {
  if (name === 'island:session') {
    const change = payload as IslandChange
    if (change.was === 'changed') alive.set(change.session.sessionId, change.session)
    else alive.delete(change.sessionId)
  }
  act(() => heard.get(name)?.(payload))
}
let peeked: { text: string | null; notShown: string | null } = { text: 'one\ntwo', notShown: null }
let askedChat: ((question: Question) => void) | undefined
let paused = { on: false, until: null as number | null }

const called = vi.fn()
vi.mock('../shell/live', () => ({
  ask: (call: () => unknown) => Promise.resolve({ data: call(), error: null, loading: false }),
  commands: {
    islandDraft: async () => null,
    islandNow: () => ({ sessions: [...alive.values()] }),
    pauseRead: async () => paused,
    islandShape: () => null,
    sessionCost: () => null,
    islandSeen: (id: string) => (called('seen', id), null),
    islandDecide: (id: string, verdict: string) => (called('decide', id, verdict), null),
    islandOpenPane: (projectId: string | null, paneId: string) => (called('open', projectId, paneId), null),
    islandOpenChat: (sessionId: string) => (called('chat', sessionId), null),
    islandPeek: () => peeked,
    islandDrop: (sessionId: string, text: string) => (called('drop', sessionId, text), null),
    islandChecks: () => ({ branch: 'fix/x', pull: { number: 7, state: 'OPEN', title: 'Fix', url: 'https://example.invalid/7' }, checks: 'passing' }),
    urlOpen: (url: string) => (called('url', url), null),
    permissionAnswer: (id: string, answer: string) => (called('answer', id, answer), null),
    permissionAlways: (id: string, sessionId: string, tool: string) => (called('always', id, sessionId, tool), null),
    permissionPick: (id: string, chosen: string[][]) => (called('pick', id, chosen), null),
  },
}))
vi.mock('../shell/window', () => ({
  onCarried: (name: string, then: (payload: unknown) => void) => (heard.set(name, then), () => heard.delete(name)),
  onPermissionAsked: (then: (question: Question) => void) => ((askedChat = then), () => {}),
  onPermissionSettled: (then: (id: string) => void) => (heard.set('permission:settled', then as (payload: unknown) => void), () => {}),
}))

const session = (over: Partial<IslandSession> = {}): IslandSession => ({
  sessionId: 's1',
  paneId: 'leaf_1',
  projectId: 'p1',
  project: 'api',
  color: '#62c987',
  cardId: null,
  card: null,
  root: '/w/api',
  state: 'working',
  steps: [{ tool: 'Edit', target: 'invoice.ts', done: false, failed: false, touch: { kind: 'edit', path: '/w/api/src/invoice.ts', before: 'a = 1', after: 'a = 2' } }],
  said: null,
  at: Date.now(),
  ...over,
})
const changed = (one: IslandSession): IslandChange => ({ was: 'changed', session: one })
const shape = (): HTMLElement => document.querySelector('.isl-shape') as HTMLElement

beforeEach(() => {
  paused = { on: false, until: null }
  peeked = { text: 'one\ntwo', notShown: null }
  heard.clear()
  alive.clear()
  called.mockClear()
})
afterEach(cleanup)

describe('the island window', () => {
  it('hides until an agent works, then shows the capsule with what it is doing', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    expect(shape().dataset.mode).toBe('hidden')
    say('island:session', changed(session()))
    expect(shape().dataset.mode).toBe('compact')
    expect(screen.getByText('Edit invoice.ts')).toBeTruthy()
  })

  it('opens on a click, lists every session, and opens one into its steps and preview', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session()))
    fireEvent.click(shape())
    expect(shape().dataset.mode).toBe('expanded')
    expect(screen.getByText('1 working')).toBeTruthy()
    fireEvent.click(screen.getByText('api'))
    expect(document.querySelector('.isl-open')?.getAttribute('data-view')).toBe('session')
    expect(screen.getByText('src/invoice.ts', { exact: false })).toBeTruthy()
    expect(document.querySelectorAll('.isl-peek__ln[data-mark="+"]').length).toBe(1)
    fireEvent.click(await screen.findByText('PR #7 open · checks passing'))
    expect(called).toHaveBeenCalledWith('url', 'https://example.invalid/7')
    fireEvent.click(screen.getByText('Open terminal'))
    expect(called).toHaveBeenCalledWith('open', 'p1', 'leaf_1')
    fireEvent.click(screen.getByLabelText('Every session'))
    expect(document.querySelector('.isl-open')?.getAttribute('data-view')).toBe('overview')
  })

  it('keeps the session being read when another one asks, and badges that one', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session()))
    say('island:session', changed(session({ sessionId: 's2', paneId: 'leaf_2', project: 'web', steps: [] })))
    fireEvent.click(shape())
    fireEvent.click(screen.getByText('api'))
    expect(document.querySelector('.isl-open')?.getAttribute('data-view')).toBe('session')

    say('island:asked', { id: 'ask_9', sessionId: 's2', paneId: 'leaf_2', project: 'web', tool: 'Bash', input: '{"command":"npm publish"}', keepable: false })
    // Still on the session being read, with the other's question as a badge.
    expect(document.querySelector('.isl-open')?.getAttribute('data-view')).toBe('session')
    expect(screen.queryByText('Run npm publish')).toBeNull()
    fireEvent.click(screen.getByText('1 waiting'))
    expect(screen.getByText('Run npm publish')).toBeTruthy()
  })

  it('a question from the session being read does take the view', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session()))
    fireEvent.click(shape())
    fireEvent.click(screen.getByText('api'))
    say('island:asked', { id: 'ask_8', sessionId: 's1', paneId: 'leaf_1', project: 'api', tool: 'Bash', input: '{"command":"ls"}', keepable: false })
    expect(screen.getByText('Run ls')).toBeTruthy()
  })

  it('shows the fold coming in its last seconds, and not while pinned', async () => {
    vi.useFakeTimers()
    try {
      render(<IslandWindow />)
      await act(async () => {})
      say('island:session', changed(session({ state: 'waiting', steps: [] })))
      expect(shape().dataset.mode).toBe('expanded')
      expect(document.querySelector('.isl-closing')).toBeNull()
      await act(async () => {
        vi.advanceTimersByTime(6_000)
      })
      expect(document.querySelector('.isl-closing')).not.toBeNull()
      fireEvent.click(screen.getByLabelText('Keep it open'))
      expect(document.querySelector('.isl-closing')).toBeNull()
    } finally {
      vi.useRealTimers()
    }
  })

  it('stays down and asks nothing while devpit is paused', async () => {
    paused = { on: true, until: null }
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session({ state: 'waiting', steps: [] })))
    expect(shape().dataset.mode).toBe('hidden')
    say('pause:changed', { on: false, until: null })
    say('island:session', changed(session({ state: 'waiting', steps: [] })))
    expect(shape().dataset.mode).toBe('expanded')
  })

  it('folds back to the capsule from the fold button, and pins open', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session()))
    fireEvent.click(shape())
    fireEvent.click(screen.getByLabelText('Keep it open'))
    expect(screen.getByLabelText('Let it fold on its own').dataset.on).toBe('true')
    fireEvent.click(screen.getByLabelText('Fold'))
    expect(shape().dataset.mode).toBe('compact')
  })

  it('opens all the way when a session waits on you', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session({ state: 'waiting', steps: [] })))
    expect(shape().dataset.mode).toBe('expanded')
    expect(shape().dataset.mood).toBe('waiting')
  })

  it('answers a terminal’s question from the island, after saying it is showing it', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    const question: IslandQuestion = { id: 'ask_1', sessionId: 's1', paneId: 'leaf_1', project: 'api', tool: 'Bash', input: '{"command":"rm -rf build"}', keepable: true }
    say('island:asked', question)
    expect(called).toHaveBeenCalledWith('seen', 'ask_1')
    expect(screen.getByText('Run rm -rf build')).toBeTruthy()
    expect(screen.getByText('api asks to')).toBeTruthy()
    fireEvent.click(screen.getByText('Always'))
    expect(called).toHaveBeenCalledWith('decide', 'ask_1', 'always')
    expect(screen.queryByText('Run rm -rf build')).toBeNull()
  })

  it('hands a terminal’s question back to the terminal when asked to', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:asked', { id: 'ask_2', sessionId: 's1', paneId: 'leaf_1', project: 'api', tool: 'Bash', input: '{}', keepable: false })
    expect(screen.queryByText('Always')).toBeNull()
    fireEvent.click(screen.getByText('Answer in terminal'))
    expect(called).toHaveBeenCalledWith('decide', 'ask_2', 'in_terminal')
  })

  it('answers a chat’s question, and puts away one answered elsewhere', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    act(() => askedChat?.({ id: 'q1', sessionId: 's9', tool: 'Edit', input: '{"file_path":"/w/a.rs"}', cwd: '/w' }))
    fireEvent.click(screen.getByText('Deny'))
    expect(called).toHaveBeenCalledWith('answer', 'q1', 'deny')

    act(() => askedChat?.({ id: 'q2', sessionId: 's9', tool: 'Edit', input: '{"file_path":"/w/b.rs"}', cwd: '/w' }))
    expect(screen.getByText('Edit /w/b.rs')).toBeTruthy()
    say('permission:settled', 'q2')
    expect(screen.queryByText('Edit /w/b.rs')).toBeNull()

    act(() => askedChat?.({ id: 'q3', sessionId: 's9', tool: 'Edit', input: '{"file_path":"/w/c.rs"}', cwd: '/w' }))
    fireEvent.click(screen.getByText('Always'))
    expect(called).toHaveBeenCalledWith('always', 'q3', 's9', 'Edit')
  })

  it('answers a chat’s question to the person by its choice, and puts it away once its chat answered it', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    const asked = (id: string): Question => ({
      id,
      sessionId: 's9',
      tool: 'AskUserQuestion',
      input: '{}',
      cwd: '/w',
      touch: { kind: 'ask', questions: [{ header: null, question: 'Which colour?', options: ['Red', 'Green', 'Blue'], multi: false }] },
    })
    act(() => askedChat?.(asked('q1')))
    expect(screen.queryByText('Always')).toBeNull()
    fireEvent.click(screen.getByText('Green'))
    expect(called).toHaveBeenCalledWith('pick', 'q1', [['Green']])
    expect(screen.queryByText('Which colour?')).toBeNull()

    act(() => askedChat?.(asked('q2')))
    expect(screen.getByText('Which colour?')).toBeTruthy()
    say('permission:settled', 'q2')
    expect(screen.queryByText('Which colour?')).toBeNull()
  })

  it('switches its sounds off and on from the head', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session()))
    fireEvent.click(shape())
    fireEvent.click(screen.getByLabelText('Mute the island'))
    expect(screen.getByLabelText('Let it make sounds')).toBeTruthy()
    expect(window.localStorage.getItem('devpit.island.sound')).toBe('off')
    fireEvent.click(screen.getByLabelText('Let it make sounds'))
    expect(window.localStorage.getItem('devpit.island.sound')).toBe('on')
  })

  it('forgets a session that has gone', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session()))
    fireEvent.click(shape())
    expect(screen.getByText('api')).toBeTruthy()
    say('island:session', { was: 'gone', sessionId: 's1' })
    expect(screen.getByText('Sessions you start in devpit show up here.')).toBeTruthy()
  })

  it('pastes the paths of files dropped on a session into that session', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session()))
    say('island:session', changed(session({ sessionId: 's2', project: 'web', paneId: 'leaf_2' })))
    fireEvent.click(shape())
    const row = document.querySelector('[data-session="s2"]') as HTMLElement
    const files = { types: ['Files'], getData: () => 'file:///w/my%20notes.md\nfile:///w/a.png' }
    fireEvent.dragOver(row, { dataTransfer: files })
    fireEvent.drop(row, { dataTransfer: files })
    expect(called).toHaveBeenCalledWith('drop', 's2', "'/w/my notes.md' /w/a.png ")
  })

  it('opens a chat session in its chat, having no terminal to open', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session({ paneId: null })))
    fireEvent.click(shape())
    fireEvent.click(screen.getByText('api'))
    fireEvent.click(screen.getByText('Open chat'))
    expect(called).toHaveBeenCalledWith('chat', 's1')
    expect(screen.queryByText('Open terminal')).toBeNull()
  })
})

describe('the island in Portuguese', () => {
  beforeEach(() => setLocale('pt-BR'))
  afterEach(() => setLocale('en'))

  it('says nobody is running once the last session has gone', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session()))
    fireEvent.click(shape())
    expect(screen.getByText('1 sessão em 1 projeto')).toBeTruthy()
    say('island:session', { was: 'gone', sessionId: 's1' })
    expect(screen.getByText('Nenhum agente rodando')).toBeTruthy()
  })

  it('counts sessions and projects in the plural', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session()))
    say('island:session', changed(session({ sessionId: 's2', projectId: 'p2', project: 'web', paneId: 'leaf_2' })))
    fireEvent.click(shape())
    expect(screen.getByText('2 sessões em 2 projetos')).toBeTruthy()
  })

  it('asks for a session to drop a file on when there is no one place for it', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session()))
    say('island:session', changed(session({ sessionId: 's2', project: 'web', paneId: 'leaf_2' })))
    fireEvent.click(shape())
    const files = { types: ['Files'], getData: () => 'file:///w/a.png' }
    fireEvent.dragOver(shape(), { dataTransfer: files })
    fireEvent.drop(shape(), { dataTransfer: files })
    expect(screen.getByText('Solte o arquivo numa sessão')).toBeTruthy()
  })

  it('says who asks, and from where', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:asked', { id: 'ask_1', sessionId: 'sx', paneId: 'leaf_1', project: null, tool: 'Bash', input: '{"command":"ls"}', keepable: false })
    expect(screen.getByText('Um agente pede para')).toBeTruthy()
    expect(screen.getByText('Do terminal · esperando sua resposta')).toBeTruthy()
  })

  it('says a preview has nothing yet before any step', async () => {
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session({ steps: [] })))
    fireEvent.click(shape())
    fireEvent.click(screen.getByText('api'))
    expect(screen.getByText('Nada ainda')).toBeTruthy()
    expect(screen.getByText('Os passos aparecem aqui conforme rodam')).toBeTruthy()
  })

  it('says a read it may not show is not shown', async () => {
    peeked = { text: null, notShown: null }
    render(<IslandWindow />)
    await act(async () => {})
    say('island:session', changed(session({ steps: [{ tool: 'Read', target: 'a.ts', done: true, failed: false, touch: { kind: 'read', path: '/w/api/a.ts', offset: null, limit: null } }] })))
    fireEvent.click(shape())
    fireEvent.click(screen.getByText('api'))
    expect(await screen.findByText('Não mostrado')).toBeTruthy()
  })
})
