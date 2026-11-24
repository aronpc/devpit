import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CheatSheet, KeysSheet } from './CheatSheet'
import { openKeysSheet } from './keysSheet'
import { doesOf, keysLabels, recordKeys, resetAllKeys, SCOPES, scopeTitle, SHORTCUT_LIST } from './shortcuts'

const openPrefs = vi.fn()
vi.mock('./useShell', () => ({ useShell: () => ({ openPrefs }) }))

afterEach(() => {
  cleanup()
  resetAllKeys()
})

describe('the cheat sheet', () => {
  it('prints every shortcut in the registry, under where it works, with its keys', () => {
    render(<KeysSheet />)
    for (const { scope } of SCOPES) {
      const group = screen.getByRole('region', { name: scopeTitle(scope) })
      for (const one of SHORTCUT_LIST.filter((shortcut) => shortcut.scope === scope)) {
        const line = within(group).getByText(doesOf(one)).closest('.ksheet__r') as HTMLElement
        expect(within(line).getAllByText(/./).map((cap) => cap.textContent)).toEqual([doesOf(one), ...keysLabels(one.id)])
      }
    }
  })

  it('prints a recorded key in place of the default', () => {
    render(<KeysSheet />)
    act(() => void recordKeys('palette', { key: 'j', ctrlKey: true }, false))
    const line = screen.getByText('Open the command palette').closest('.ksheet__r') as HTMLElement
    expect(within(line).getByText('Ctrl+J')).toBeTruthy()
    expect(within(line).queryByText('Ctrl+K')).toBeNull()
  })

  it('opens on ? and on Ctrl+/, and closes on Escape', () => {
    render(<CheatSheet />)
    fireEvent.keyDown(document.body, { key: '?', shiftKey: true })
    expect(screen.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeTruthy()
    fireEvent.keyDown(document.body, { key: 'Escape' })
    expect(screen.queryByRole('dialog')).toBeNull()
    fireEvent.keyDown(document.body, { key: '/', ctrlKey: true })
    expect(screen.getByRole('dialog')).toBeTruthy()
  })

  it('opens from the palette, which has no key to press', () => {
    render(<CheatSheet />)
    act(openKeysSheet)
    expect(screen.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeTruthy()
  })

  it('stays shut on a ? typed into a field', () => {
    render(
      <>
        <CheatSheet />
        <input aria-label="Title" />
      </>,
    )
    fireEvent.keyDown(screen.getByLabelText('Title'), { key: '?', shiftKey: true })
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('leads to Settings to change them', () => {
    render(<CheatSheet />)
    fireEvent.keyDown(document.body, { key: '?', shiftKey: true })
    fireEvent.click(screen.getByText('Change them in Settings'))
    expect(openPrefs).toHaveBeenCalledWith('keyboard')
    expect(screen.queryByRole('dialog')).toBeNull()
  })
})
