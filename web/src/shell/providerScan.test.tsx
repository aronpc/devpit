import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { ProviderFound, ProvidersFound } from '../gen/bindings'

afterEach(cleanup)

let found: ProvidersFound = { providers: [], profiles: [] }
const scanned: boolean[] = []
const shown = vi.fn()
const closed = vi.fn()
let project: { id: string } | null = { id: 'prj_a' }

vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve({ data: call(), error: null, loading: false }),
  commands: {
    providersDetect: (again: boolean) => (scanned.push(again), found),
    providersCheckRead: () => ({ minutes: 5 }),
  },
}))
vi.mock('./useShell', () => ({ useShell: () => ({ project, show: shown, closePrefs: closed }) }))

const { ProviderScan } = await import('./ProviderScan')

const provider = (id: string, over: Partial<ProviderFound> = {}): ProviderFound => ({
  id,
  label: id,
  path: null,
  version: null,
  signedIn: 'unknown',
  chat: false,
  install: `install ${id}`,
  login: `${id} login`,
  homepage: '',
  ...over,
})

beforeEach(() => {
  scanned.length = 0
  shown.mockClear()
  closed.mockClear()
  project = { id: 'prj_a' }
  found = {
    providers: [
      provider('claude', { label: 'Claude Code', path: '/bin/claude', version: '2.1.325', signedIn: 'saved', chat: true }),
      provider('codex', { label: 'Codex', path: '/bin/codex', version: '0.160.0', signedIn: 'missing' }),
      provider('opencode', { label: 'OpenCode', path: '/bin/opencode', version: '1.17.4', signedIn: 'saved' }),
      provider('grok', { label: 'Grok', install: 'curl -fsSL https://x.ai/cli/install.sh | bash' }),
    ],
    profiles: [{ id: 'codex-work', label: 'Codex (work)', base: 'codex', signedIn: 'missing' }],
  }
})

const row = (id: string): HTMLElement => document.querySelector(`[data-provider="${id}"]`) as HTMLElement

describe('Settings → Providers', () => {
  it('lists the installed CLIs with their versions and a missing one with its install command', async () => {
    render(<ProviderScan />)
    await waitFor(() => expect(row('claude')).toBeTruthy())
    expect(row('claude').textContent).toContain('2.1.325')
    expect(row('codex').textContent).toContain('0.160.0')
    expect(row('opencode').textContent).toContain('1.17.4')
    expect(row('grok').textContent).toContain('not installed')
    expect(row('grok').textContent).toContain('curl -fsSL https://x.ai/cli/install.sh | bash')
    expect(row('claude').textContent).toContain('Chat')
    expect(row('codex').textContent).toContain('Terminal only')
    expect(scanned).toEqual([false])
  })

  it('shows a signed-out CLI its login, and Sign in opens it in a terminal tab', async () => {
    render(<ProviderScan />)
    await waitFor(() => expect(row('codex')).toBeTruthy())
    expect(row('codex').textContent).toContain('Signed out')
    expect(row('codex').textContent).toContain('codex login')
    expect(row('claude').textContent).not.toContain('Sign in')
    fireEvent.click(screen.getAllByRole('button', { name: 'Sign in' })[0])
    expect(shown).toHaveBeenCalledWith('term', { title: 'Sign in · Codex', login: 'codex' })
    expect(closed).toHaveBeenCalled()
  })

  it('signs a second profile of the same CLI in as itself', async () => {
    render(<ProviderScan />)
    await waitFor(() => expect(screen.getByText('Codex (work)')).toBeTruthy())
    fireEvent.click(screen.getAllByRole('button', { name: 'Sign in' })[1])
    expect(shown).toHaveBeenCalledWith('term', { title: 'Sign in · Codex (work)', login: 'codex-work' })
  })

  it('turns signed-in after Scan again', async () => {
    render(<ProviderScan />)
    await waitFor(() => expect(row('codex').textContent).toContain('Signed out'))
    found = {
      ...found,
      providers: found.providers.map((one) => (one.id === 'codex' ? { ...one, signedIn: 'saved' } : one)),
    }
    fireEvent.click(screen.getByRole('button', { name: 'Scan again' }))
    await waitFor(() => expect(row('codex').textContent).toContain('Signed in'))
    expect(scanned).toEqual([false, true])
  })

  it('asks for a project before a sign-in, since it runs in one of its terminals', async () => {
    project = null
    render(<ProviderScan />)
    await waitFor(() => expect(row('codex')).toBeTruthy())
    fireEvent.click(screen.getAllByRole('button', { name: 'Sign in' })[0])
    expect(shown).not.toHaveBeenCalled()
    expect(screen.getByText(/Open a project first/)).toBeTruthy()
  })
})
