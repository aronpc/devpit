import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { LiveSession } from '../gen/bindings'
import { setLocale } from '../i18n'
import { since, SessionsView } from './SessionsView'

const replied = vi.fn()
const opened = vi.fn()
const live = (name: string, projectName: string, extra: Partial<LiveSession> = {}): LiveSession =>
  ({ name, status: 'idle', kind: 'interactive', cwd: '/w', projectId: 'p', projectName, cardId: null, since: null, inDevpit: true, waiting: null, pane: null, remote: { on: false, url: null }, ...extra }) as LiveSession

vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve({ data: call(), error: null, loading: false }),
  commands: {
    orchestratorSessions: () => ({
      sessions: [
        live('api-b', 'api'),
        live('web-a', 'web', { status: 'busy', projectId: 'w', step: 'Edit invoice.ts', pane: { projectId: 'w', paneId: 'leaf_2' }, remote: { on: true, url: 'https://claude.ai/code/session_01W' } }),
        live('api-a', 'api', { waiting: { question: 'Deploy now?', options: [], cursor: 0 }, pane: { projectId: 'p', paneId: 'leaf_1' } }),
      ],
    }),
    orchestratorAgents: () => [
      { name: 'api-a', lastAt: '2026-09-25T18:13:45Z', events: [{ kind: 'sent', at: '2026-09-25T18:11:55Z', summary: 'Deploy the fix', text: 'Deploy it to dev.' }] },
      { name: 'old-one', lastAt: '2026-09-20T10:00:00Z', events: [{ kind: 'heard', at: '2026-09-20T10:00:00Z', summary: null, text: 'Done.' }] },
    ],
    orchestratorReply: (...args: unknown[]) => (replied(...args), null),
    orchestratorLinks: () => ['p'],
    orchestratorStop: () => 'stopped',
    orchestratorEnded: () => ({ sessions: [] }),
    urlOpen: (url: string) => (opened(url), { opened: true }),
  },
}))
vi.mock('./useShell', () => ({
  useShell: () => ({ project: { id: 'orch', orchestrator: 'claude' }, projects: [{ id: 'p', name: 'api', rootPath: '/w/api' }], setProject: () => {}, show: () => {}, openCard: () => {}, openPane: () => {} }),
}))

afterEach(() => {
  cleanup()
  replied.mockClear()
  opened.mockClear()
})

describe("an orchestrator's sessions", () => {
  it('group by project, the one waiting on the person first, with what was last asked of it', async () => {
    render(<SessionsView shown />)
    const names = (await screen.findAllByText(/^(api|web)-[ab]$/)).map((one) => one.textContent)
    expect(names).toEqual(['api-a', 'api-b', 'web-a'])
    expect(screen.getByText('1 waiting')).toBeTruthy()
    expect(await screen.findByText(/Deploy the fix/)).toBeTruthy()
    expect(screen.getByLabelText("Open api-a's terminal here")).toBeTruthy()
    expect(screen.queryByLabelText("Open api-b's terminal here")).toBeNull()
  })

  it('opens to the question, a reply typed as the person, and the history', async () => {
    render(<SessionsView shown />)
    fireEvent.click(await screen.findByText('api-a'))
    expect(screen.getByText('Deploy now?')).toBeTruthy()
    expect(screen.getByText('Deploy it to dev.')).toBeTruthy()
    const input = screen.getByLabelText('Reply to api-a')
    fireEvent.change(input, { target: { value: 'yes, go' } })
    fireEvent.submit(input.closest('form')!)
    expect(replied).toHaveBeenCalledWith('claude', 'api-a', 'yes, go', 'the Sessions panel')
  })

  it('sends a command to a session in a devpit terminal only once it is confirmed, typed as the person', async () => {
    render(<SessionsView shown />)
    fireEvent.click(await screen.findByText('api-b'))
    expect(screen.queryByRole('button', { name: 'Send command…' })).toBeNull()
    cleanup()
    render(<SessionsView shown />)
    fireEvent.click(await screen.findByText('web-a'))
    fireEvent.click(screen.getByRole('button', { name: 'Send command…' }))
    fireEvent.click(screen.getByRole('button', { name: '/remote-control' }))
    expect(replied).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Send' }))
    expect(replied).toHaveBeenCalledWith('claude', 'web-a', '/remote-control', 'the Sessions panel')
  })

  it('sends a command written by hand, after the same question', async () => {
    render(<SessionsView shown />)
    fireEvent.click(await screen.findByText('web-a'))
    fireEvent.click(screen.getByRole('button', { name: 'Send command…' }))
    const input = screen.getByLabelText('A command for web-a')
    fireEvent.change(input, { target: { value: '  /model   opus ' } })
    fireEvent.submit(input.closest('form')!)
    expect(replied).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Send' }))
    expect(replied).toHaveBeenCalledWith('claude', 'web-a', '/model opus', 'the Sessions panel')
  })

  it('says on each row whether Remote Control is on, and opens its page when it is', async () => {
    render(<SessionsView shown />)
    fireEvent.click(await screen.findByRole('button', { name: 'Open web-a on claude.ai' }))
    expect(opened).toHaveBeenCalledWith('https://claude.ai/code/session_01W')
    expect(screen.getByRole('img', { name: 'Remote Control off for api-b' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Open api-b on claude.ai' })).toBeNull()
  })

  it('says what a busy session is on right now', async () => {
    render(<SessionsView shown />)
    expect(await screen.findByText(/· Edit invoice\.ts/)).toBeTruthy()
  })

  it('keeps the ones no longer running below, with their history', async () => {
    render(<SessionsView shown />)
    expect(await screen.findByText('Earlier · not running')).toBeTruthy()
    fireEvent.click(screen.getByText('old-one'))
    expect(screen.getByText('Done.')).toBeTruthy()
  })

  it('says how long in the language chosen', () => {
    const now = Date.UTC(2026, 8, 25, 12)
    setLocale('pt-BR')
    try {
      expect(since(now - 20_000, now)).toBe('agora há pouco')
      expect(since(now - 2 * 60_000, now)).toBe('há 2 minutos')
    } finally {
      setLocale('en')
    }
  })

  it('says how long in words', () => {
    const now = Date.UTC(2026, 8, 25, 12)
    expect(since(null, now)).toBeNull()
    expect(since(now - 20_000, now)).toBe('just now')
    expect(since(now - 12 * 60_000, now)).toBe('12 minutes ago')
    expect(since(now - 3 * 3_600_000, now)).toBe('3 hours ago')
  })
})
