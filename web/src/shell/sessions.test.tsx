import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { SessionRows } from './SessionRowsView'
import type { Tab } from './strip'

afterEach(cleanup)

const shell = {
  open: [] as Tab[],
  active: null as Tab | null,
  running: [],
  focus: vi.fn(),
  close: vi.fn(),
  rename: vi.fn(),
  renaming: null as { id: string; where: string } | null,
  setRenaming: vi.fn(),
  unread: new Set<string>(),
  threads: [],
  profiles: [],
  chatDoing: {},
  mark: vi.fn(),
}

vi.mock('./useShell', () => ({ useShell: () => shell }))

const chat: Tab = { id: 'c1', kind: 'chat', title: 'tudo bem?' }

describe('the sessions in the sidebar', () => {
  it('opens the one you clicked', () => {
    shell.open = [chat]
    shell.renaming = null
    render(<SessionRows />)
    fireEvent.click(screen.getByText('tudo bem?'))
    expect(shell.focus).toHaveBeenCalledWith('c1')
  })

  it('starts a rename on two quick clicks', () => {
    /* The interaction the browser never reported for us: a captured pointer
       retargets the events `dblclick` is derived from. */
    shell.open = [chat]
    shell.renaming = null
    shell.setRenaming.mockClear()
    render(<SessionRows />)
    const row = screen.getByText('tudo bem?')
    /* jsdom gives every synthetic event the same timestamp, so the window
       itself is tested against real numbers in `strip.test.ts`. What this
       covers is that two clicks reach the rule at all. */
    fireEvent.click(row)
    fireEvent.click(row)
    expect(shell.setRenaming).toHaveBeenCalledWith({ id: 'c1', where: 'sidebar' })
  })

  it('leaves the strip alone when the strip is the one renaming', () => {
    /* Both surfaces draw the same tab. Two fields race for the focus, and the
       one that loses it commits and shuts the other — which is what made this
       look like a feature that did nothing at all. */
    shell.open = [chat]
    shell.renaming = { id: 'c1', where: 'strip' }
    render(<SessionRows />)
    expect(screen.queryByRole('textbox')).toBeNull()
  })

  it('draws a field for the one being renamed', () => {
    shell.open = [chat]
    shell.renaming = { id: 'c1', where: 'sidebar' }
    render(<SessionRows />)
    expect((screen.getByRole('textbox') as HTMLInputElement).value).toBe('tudo bem?')
  })

  it('hands the new name to the shell', () => {
    shell.open = [chat]
    shell.renaming = { id: 'c1', where: 'sidebar' }
    shell.rename.mockClear()
    render(<SessionRows />)
    const field = screen.getByRole('textbox')
    fireEvent.change(field, { target: { value: 'deploy notes' } })
    fireEvent.keyDown(field, { key: 'Enter' })
    expect(shell.rename).toHaveBeenCalledWith('c1', 'deploy notes')
  })

  it('cuts a long name to fit the column', () => {
    shell.open = [{ id: 'c2', kind: 'chat', title: 'x'.repeat(60) }]
    shell.renaming = null
    render(<SessionRows />)
    expect(screen.getByText(`${'x'.repeat(30)}…`)).toBeTruthy()
  })

  it('shows what a session is doing as a chip: a mark and a word', () => {
    shell.open = [chat]
    shell.renaming = null
    shell.chatDoing = { c1: 'waiting' }
    render(<SessionRows />)
    const chip = document.querySelector('.stchip')
    expect(chip?.getAttribute('data-doing')).toBe('waiting')
    expect(chip?.querySelector('.card__ask')).not.toBeNull()
    expect(chip?.textContent).toContain('waiting on you')
    shell.chatDoing = {}
  })
})
