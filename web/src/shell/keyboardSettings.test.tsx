import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { KeyboardSettings } from './KeyboardSettings'
import { doesOf, keysLabel, resetAllKeys, SHORTCUT_LIST } from './shortcuts'

afterEach(() => {
  cleanup()
  resetAllKeys()
})

const row = (does: string): HTMLElement => screen.getByText(does).closest<HTMLElement>('.kmap__row')!

describe('Settings → Keyboard', () => {
  it('lists every shortcut, with the keys it answers to now', () => {
    render(<KeyboardSettings onGeneral={vi.fn()} />)
    for (const one of SHORTCUT_LIST) expect(screen.getByText(doesOf(one))).toBeTruthy()
    expect(within(row('Open the command palette')).getByText(keysLabel('palette'))).toBeTruthy()
  })

  it('lists the keys that read a conversation, under Chat', () => {
    render(<KeyboardSettings onGeneral={vi.fn()} />)
    for (const [does, id] of [
      ['Go to the next message', 'nextMessage'],
      ['Go to the previous message', 'previousMessage'],
      ['Go to the next tool that failed', 'nextFailed'],
      ['Open or close the row you are on', 'toggleRow'],
    ] as const) {
      expect(within(row(does)).getByText(keysLabel(id))).toBeTruthy()
    }
  })

  it('finds a shortcut by what it does or by its keys', () => {
    render(<KeyboardSettings onGeneral={vi.fn()} />)
    const search = screen.getByLabelText('Search shortcuts')
    fireEvent.change(search, { target: { value: 'split' } })
    expect(screen.getByText('Split the terminal to the right')).toBeTruthy()
    expect(screen.queryByText('Open the command palette')).toBeNull()
    fireEvent.change(search, { target: { value: keysLabel('palette') } })
    expect(screen.getByText('Open the command palette')).toBeTruthy()
  })

  it('records a key, refusing one that is taken and naming who has it', () => {
    render(<KeyboardSettings onGeneral={vi.fn()} />)
    fireEvent.click(within(row('Open the command palette')).getByText('Record'))
    expect(screen.getByText('Press the keys — Esc to stop')).toBeTruthy()

    fireEvent.keyDown(window, { key: 't', ctrlKey: true })
    expect(screen.getByText(`${keysLabel('terminal')} is already “New terminal”.`)).toBeTruthy()

    fireEvent.keyDown(window, { key: 'j', ctrlKey: true })
    expect(within(row('Open the command palette')).getByText('Ctrl+J')).toBeTruthy()

    fireEvent.click(within(row('Open the command palette')).getByText('Reset'))
    expect(within(row('Open the command palette')).getByText('Ctrl+K')).toBeTruthy()
  })

  it('stops recording on Escape, without Settings hearing it', () => {
    const settings = vi.fn()
    document.addEventListener('keydown', settings)
    render(<KeyboardSettings onGeneral={vi.fn()} />)
    const record = within(row('Split the terminal down')).getByText('Record')
    fireEvent.click(record)
    fireEvent.keyDown(record, { key: 'Escape' })
    document.removeEventListener('keydown', settings)
    expect(settings).not.toHaveBeenCalled()
    expect(screen.queryByText('Press the keys — Esc to stop')).toBeNull()
    expect(keysLabel('splitDown')).toBe('Ctrl+Shift+E')
  })

  it('lists a built-in key without a way to change it', () => {
    render(<KeyboardSettings onGeneral={vi.fn()} />)
    expect(within(row('Open the focused card')).queryByText('Record')).toBeNull()
  })

  it('sends the desktop-wide key to General, where it is set', () => {
    const onGeneral = vi.fn()
    render(<KeyboardSettings onGeneral={onGeneral} />)
    fireEvent.click(screen.getByText('Open General'))
    expect(onGeneral).toHaveBeenCalled()
  })
})
