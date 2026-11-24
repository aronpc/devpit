import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { EndedSession } from '../gen/bindings'
import { wouldLose } from './SessionsView'

const resumed = vi.fn()
const ended = (name: string, extra: Partial<EndedSession> = {}): EndedSession => ({
  sessionId: `id-${name}`,
  name,
  cwd: '/w/api',
  projectId: 'p',
  projectName: 'api',
  cardId: null,
  startedAt: 1,
  endedAt: 2,
  endedBy: null,
  lastStatus: 'idle',
  dirty: null,
  ...extra,
})

vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: {
    orchestratorEnded: () => ({
      sessions: [ended('a7', { endedBy: 'orchestrator', lastStatus: 'busy', dirty: 12 }), ended('b2', { lastStatus: 'busy' })],
    }),
    orchestratorTold: () => ({ replies: ['Phase 1: halfway, tests next.'], prompts: [] }),
    orchestratorResume: (...args: unknown[]) => (resumed(...args), 'a7'),
  },
}))
vi.mock('./liveStatus', () => ({ refreshSessions: () => {} }))

const { EndedSessions, endedHow } = await import('./EndedSessions')

afterEach(cleanup)

describe('sessions that ended', () => {
  it('say how they ended', () => {
    expect(endedHow(ended('x', { endedBy: 'person' }))).toBe('stopped by you')
    expect(endedHow(ended('x', { endedBy: 'orchestrator' }))).toBe('stopped by the orchestrator')
    expect(endedHow(ended('x', { lastStatus: 'busy' }))).toBe('ended while working')
  })

  it('are listed with what they left, read, and resumed as the same conversation', async () => {
    render(<EndedSessions profileId="claude" running={0} />)
    fireEvent.click(await screen.findByText('Ended · 2'))
    expect(screen.getByText(/12 files not committed/)).toBeTruthy()
    fireEvent.click(screen.getByText('a7'))
    expect(await screen.findByText('Phase 1: halfway, tests next.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Resume' }))
    expect(resumed).toHaveBeenCalledWith('claude', 'id-a7')
    expect(await screen.findByText(/Resumed as a7/)).toBeTruthy()
  })

  it('a stop that would lose work asks again', () => {
    expect(wouldLose('a7: it is working right now — stop it anyway?')).toBe(true)
    expect(wouldLose('no session called a7 is running')).toBe(false)
    expect(wouldLose(null)).toBe(false)
  })
})
