import { cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Thread } from '../gen/bindings'
import { setLocale } from '../i18n'
import { NO_MARKS } from './useConversations'
import { SessionRows } from './SessionRowsView'
import type { Tab } from './strip'

afterEach(() => {
  cleanup()
  setLocale('en')
})

const NOW = Math.floor(Date.now() / 1000)
const thread = (id: string, title: string, marks = NO_MARKS, cwd: string | null = null): Thread => ({ id, title, profile: 'claude', model: null, costUsd: 0, lastAt: NOW - 120, cwd, marks })

const shell = {
  open: [
    { id: 'c1', kind: 'chat', title: 'Fix the lexer' },
    { id: 'c2', kind: 'chat', title: 'Old release notes' },
    { id: 'c3', kind: 'chat', title: 'Card work' },
    { id: 't1', kind: 'term', title: 'Terminal' },
  ] as Tab[],
  active: null,
  running: [],
  doing: {},
  unread: new Set<string>(),
  subagents: {},
  focus: vi.fn(),
  close: vi.fn(),
  rename: vi.fn(),
  renaming: null,
  setRenaming: vi.fn(),
  project: { id: 'prj_1', name: 'devpit', rootPath: '/work/devpit' },
  threads: [
    thread('c1', 'Fix the lexer', { ...NO_MARKS, pinned: true }),
    thread('c2', 'Old release notes', { ...NO_MARKS, settled: true }),
    thread('c3', 'Card work', NO_MARKS, '/work/devpit-card'),
  ],
  profiles: [{ id: 'claude', label: 'Claude Code', driver: 'claude' }],
  chatDoing: { c3: 'waiting' },
  mark: vi.fn(),
}

vi.mock('./useShell', () => ({ useShell: () => shell }))
vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve({ data: call(), error: null }),
  commands: { folderGlance: (folder: string) => ({ branch: folder.endsWith('card') ? 'feat/325-card' : 'main', files: 0, added: 0, removed: 0 }) },
}))

const rowOf = (title: string): HTMLElement => screen.getByText(title).closest<HTMLElement>('.card')!

describe('the sidebar’s conversations', () => {
  it('are grouped as in progress and settled, each with its count', () => {
    render(<SessionRows />)
    expect(screen.getByText('In progress').textContent).toBe('In progress 3')
    expect(screen.getByText('Settled').textContent).toBe('Settled 1')
    const titles = [...document.querySelectorAll('.card__t')].map((one) => one.textContent)
    expect(titles).toEqual(['Fix the lexer', 'Card work', 'Terminal', 'Old release notes'])
  })

  it('say each one’s provider, state, branch, project and when it was last spoken in', async () => {
    render(<SessionRows />)
    const card = rowOf('Card work')
    expect(within(card).getByText('Claude Code · waiting on you')).toBeTruthy()
    expect(await within(card).findByText('feat/325-card · devpit · 2 minutes ago')).toBeTruthy()
    expect(await within(rowOf('Fix the lexer')).findByText('main · devpit · 2 minutes ago')).toBeTruthy()
    expect(within(rowOf('Fix the lexer')).getByText('Claude Code')).toBeTruthy()
    expect(within(rowOf('Fix the lexer')).getByLabelText('Pinned')).toBeTruthy()
  })

  it('say when it was last spoken in, in the language chosen', async () => {
    setLocale('pt-BR')
    render(<SessionRows />)
    expect(await within(rowOf('Card work')).findByText('feat/325-card · devpit · há 2 minutos')).toBeTruthy()
    expect(within(rowOf('Card work')).getByText('Claude Code · esperando por você')).toBeTruthy()
    expect(screen.getByText('Em andamento').textContent).toBe('Em andamento 3')
  })
})
