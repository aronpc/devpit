import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { SessionOnCard } from '../gen/bindings'
import { setLocale } from '../i18n'
import { CardSessionHistory, resumable, sessionWords } from './CardSessionHistory'

const resumed = vi.fn()
const setProject = vi.fn()
const openPane = vi.fn()

const seen = (name: string, extra: Partial<SessionOnCard> = {}): SessionOnCard => ({
  profileId: 'claude',
  sessionId: `id-${name}`,
  name,
  startedAt: Date.UTC(2026, 9, 10, 9),
  endedAt: null,
  endedBy: null,
  lastStatus: 'idle',
  dirty: null,
  pane: null,
  remote: { on: false, url: null },
  ...extra,
})

const running = seen('api-live', { pane: { projectId: 'p', paneId: 'leaf_1' }, remote: { on: true, url: 'https://claude.ai/code/session_01L' } })
const ended = seen('api-done', { endedAt: Date.UTC(2026, 9, 10, 11), endedBy: 'person', dirty: 2 })

vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve({ data: call(), error: null, loading: false }),
  commands: {
    cardSessionHistory: () => ({ sessions: [running, ended] }),
    orchestratorResume: (...args: unknown[]) => (resumed(...args), 'api-done'),
  },
}))
vi.mock('./useShell', () => ({
  useShell: () => ({ setProject, openPane }),
}))

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  setLocale('en')
})

describe("a card's session history", () => {
  it('offers Resume for an ended session only', () => {
    expect(resumable(ended)).toBe(true)
    expect(resumable(running)).toBe(false)
  })

  it('says how each stands, and what it left uncommitted', () => {
    expect(sessionWords(running)).toMatch(/^running since /)
    expect(sessionWords(ended)).toMatch(/^stopped by you .+ · 2 files not committed$/)
    expect(sessionWords({ ...ended, dirty: 1 })).toMatch(/ · 1 file not committed$/)
  })

  /* Noon UTC on 20 January: the same day on any runner, and past the twelfth,
     so day-first and month-first cannot pass for each other. The machine's
     locale is not the person's: under LC_ALL=en_US the app in pt-BR still
     says 20/01/2026. */
  const january = { ...ended, endedAt: Date.UTC(2026, 0, 20, 12), dirty: null }

  it('dates a session in the chosen locale', () => {
    setLocale('pt-BR')
    expect(sessionWords(january)).toMatch(/ 20\/01\/2026/)
  })

  it('dates a session in English when English is chosen', () => {
    expect(sessionWords(january)).toMatch(/ 1\/20\/26/)
  })

  it('resumes an ended one as the same conversation, and goes to a running one', async () => {
    render(<CardSessionHistory cardId="card_a" running={1} />)
    const rows = (await screen.findAllByText(/^api-/)).map((one) => one.textContent)
    expect(rows).toEqual(['api-live', 'api-done'])
    expect(screen.getAllByRole('button', { name: 'Resume' })).toHaveLength(1)
    expect(screen.getAllByRole('button', { name: 'Go to' })).toHaveLength(1)
    expect(screen.getByRole('button', { name: 'Open api-live on claude.ai' })).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'Resume' }))
    expect(resumed).toHaveBeenCalledWith('claude', 'id-api-done')
    fireEvent.click(screen.getByRole('button', { name: 'Go to' }))
    expect(setProject).toHaveBeenCalledWith('p')
    expect(openPane).toHaveBeenCalledWith('leaf_1')
  })
})
