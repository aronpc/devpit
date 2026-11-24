import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { setLocale } from '../i18n'
import { chatMessages } from './chatMessages'
import { ChatPane } from './ChatPane'
import type { Tab } from './strip'

afterEach(() => {
  cleanup()
  setLocale('en')
})

const chat = {
  messages: chatMessages(),
  rewindable: [],
  files: [],
  skills: [],
  previews: {},
  asked: [],
  error: null,
  sending: false,
  profileId: 'prof_1',
  profiles: [],
  cost: 0,
  session: null,
  card: null,
  say: vi.fn(),
  stop: vi.fn(),
  answer: vi.fn(),
  detach: vi.fn(),
  attach: vi.fn(),
  paste: vi.fn(),
  setSkills: vi.fn(),
  rewind: vi.fn(),
}

vi.mock('./useChat', () => ({ useChat: () => chat }))
vi.mock('./useShell', () => ({
  useShell: () => ({
    active: { id: 'conv_1' },
    project: { id: 'p1', name: 'devpit' },
    rename: vi.fn(),
    drafted: vi.fn(),
    finished: vi.fn(),
    show: vi.fn(),
    threads: [],
    mark: vi.fn(),
    reload: vi.fn(),
    tellDoing: vi.fn(),
  }),
}))
vi.mock('./ChatHead', () => ({
  ChatHead: ({ title }: { title: string }) => <h1>{title}</h1>,
}))
vi.mock('./ChatBlank', () => ({ ChatBlank: () => null }))
vi.mock('./Composer', () => ({ Composer: () => null }))
vi.mock('./DropTarget', () => ({ DropTarget: () => null }))
vi.mock('./Quoting', () => ({ Quoting: () => null }))

function renderTitle(locale: string, tab: Tab): void {
  setLocale(locale)
  render(<ChatPane tab={tab} />)
}

describe('the chat pane title', () => {
  it('shows the Portuguese fallback for an unnamed chat', () => {
    renderTitle('pt-BR', { id: 'conv_1', kind: 'chat' })
    expect(screen.getByRole('heading', { name: 'Novo chat' })).toBeTruthy()
  })

  it('shows the English fallback for an unnamed chat', () => {
    renderTitle('en', { id: 'conv_1', kind: 'chat' })
    expect(screen.getByRole('heading', { name: 'New chat' })).toBeTruthy()
  })

  it('keeps the title of a named chat', () => {
    renderTitle('pt-BR', { id: 'conv_1', kind: 'chat', title: 'Release notes' })
    expect(screen.getByRole('heading', { name: 'Release notes' })).toBeTruthy()
  })
})
