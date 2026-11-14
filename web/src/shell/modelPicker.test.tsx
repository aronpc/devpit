import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { Profile } from '../gen/bindings'
import { ModelPicker } from './ModelPicker'

afterEach(cleanup)

const openPrefs = vi.fn()
vi.mock('./useShell', () => ({ useShell: () => ({ project: { id: 'prj_1' }, openPrefs }) }))
vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve({ data: call(), error: null }),
  commands: {
    cliInstallations: () => [
      { directory: '/home/me/.claude', profiles: [], ids: [], default: true },
      { directory: '/home/me/.claude-2', profiles: ['claude2'], ids: ['p_claude2'], default: false },
    ],
    skillsList: (directory: string) => ({ skills: directory.endsWith('claude-2') ? [{}, {}, {}] : [{}], problem: null, directory }),
    mcpList: (_: string, directory: string) => ({ servers: [{}], sources: [], problem: null, manageWith: '', directory }),
  },
}))

const profile = (over: Partial<Profile>): Profile =>
  ({ id: 'p', label: 'p', command: 'claude', driver: 'claude', path: '/bin/claude', reach: 'runnable', base: 'claude', args: [], env: [], mine: true, models: ['default', 'opus', 'sonnet'], efforts: [], effortDefault: null, ...over }) as Profile

const claude = profile({ id: 'claude', label: 'Claude Code', mine: false, base: '' })
const claude2 = profile({
  id: 'p_claude2',
  label: 'claude2',
  env: [{ name: 'CLAUDE_CONFIG_DIR', value: '/home/me/.claude-2' }],
})

beforeEach(() => {
  openPrefs.mockClear()
  localStorage.clear()
})

function opened(profiles: Profile[], profileId: string | null, onPick = vi.fn()): typeof onPick {
  render(<ModelPicker profiles={profiles} profileId={profileId} model={null} fixed={null} onPick={onPick} />)
  fireEvent.click(screen.getByLabelText('Account and model'))
  return onPick
}

describe('the model picker', () => {
  /* Two profiles of one CLI wear one mark; the rail has to say which is which. */
  it('names every account on the rail, with what makes it that account', () => {
    opened([claude, claude2], 'claude')
    const rail = screen.getByRole('navigation', { name: 'Accounts' })
    expect(within(rail).getByText('claude2')).toBeTruthy()
    expect(within(rail).getByText('.claude-2')).toBeTruthy()
    expect(within(rail).getByText('Default sign-in')).toBeTruthy()
  })

  it('shows versions, what is sent, and which one this chat is on', () => {
    opened([claude], 'claude')
    const current = screen.getByRole('option', { selected: true })
    expect(within(current).getByText('Account default')).toBeTruthy()
    const opus = screen.getByText('Opus 5.5').closest('[role="option"]') as HTMLElement
    // What it is for beside the name; the alias sent, on hover.
    expect(within(opus).getByText('Most capable')).toBeTruthy()
    expect(within(opus).getByTitle('opus')).toBeTruthy()
  })

  /* An agent that lists its own models names them; one it refused says why
     and cannot be picked. */
  it('names the models its agent offered, and shows one it refused as unavailable with why', () => {
    const cursor = profile({
      id: 'cursor',
      label: 'Cursor',
      command: 'cursor-agent',
      driver: 'acp',
      base: 'cursor',
      models: ['default', 'composer-2.5[fast=false]', 'gpt-5'],
      offered: [
        { id: 'composer-2.5[fast=false]', name: 'Composer 2.5', unavailable: null },
        { id: 'gpt-5', name: 'gpt-5', unavailable: 'Invalid model value: gpt-5' },
      ],
    })
    const onPick = opened([cursor], 'cursor')
    expect(screen.getByText('Composer 2.5')).toBeTruthy()
    const refused = screen.getByText('gpt-5').closest('[role="option"]') as HTMLElement
    expect(refused.getAttribute('aria-disabled')).toBe('true')
    expect(within(refused).getByText('Invalid model value: gpt-5')).toBeTruthy()
    fireEvent.click(refused)
    expect(onPick).not.toHaveBeenCalled()
  })

  it('picks the account and the model together', () => {
    const onPick = opened([claude, claude2], 'claude')
    fireEvent.click(within(screen.getByRole('navigation', { name: 'Accounts' })).getByText('claude2'))
    fireEvent.click(screen.getByText('Sonnet 5'))
    expect(onPick).toHaveBeenCalledWith('p_claude2', 'sonnet')
  })

  it('moves along the rail with the arrows while the search is empty', () => {
    const onPick = opened([claude, claude2], 'claude')
    const field = screen.getByLabelText('Search models')
    fireEvent.keyDown(field, { key: 'ArrowRight' })
    fireEvent.keyDown(field, { key: 'ArrowDown' })
    fireEvent.keyDown(field, { key: 'Enter' })
    expect(onPick).toHaveBeenCalledWith('p_claude2', 'opus')
  })

  /* What changes with the account: where its history, skills and MCP live. */
  it('says where the account on the rail keeps its configuration', async () => {
    opened([claude, claude2], 'claude')
    fireEvent.click(within(screen.getByRole('navigation', { name: 'Accounts' })).getByText('claude2'))
    await waitFor(() => expect(screen.getByText('~/.claude-2')).toBeTruthy())
    expect(await screen.findByText('3 skills · 1 MCP server')).toBeTruthy()
  })

  it('points at Providers when there is only one account', () => {
    opened([claude], 'claude')
    fireEvent.click(screen.getByText('+ Add an account'))
    expect(openPrefs).toHaveBeenCalledWith('providers')
  })
})
