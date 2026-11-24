import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ArchivedCards, CardDeleted } from '../gen/bindings'
import { setLocale } from '../i18n'
import { ArchivedPane, UNDO_MS, UndoArchive } from './BoardShelf'

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  setLocale('en')
})

/* Noon UTC on 20 January: the same day on any runner, and a day past the
   twelfth, so a day-first date and a month-first one cannot pass for each
   other. */
const ARCHIVED_AT = Date.UTC(2026, 0, 20, 12) / 1000

const restored = vi.fn()
const deleted = vi.fn()

vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve({ data: call(), error: null, loading: false }),
  commands: {
    boardArchived: (): ArchivedCards => ({
      cards: [{ id: 'card_a', title: 'Old idea', columnName: 'Todo', archivedAt: ARCHIVED_AT }],
    }),
    cardRestore: (_project: string, card: string) => {
      restored(card)
      return null
    },
    cardDelete: (_project: string, card: string, force: boolean): CardDeleted => {
      deleted(card, force)
      return { deleted: true, refused: null }
    },
    runsList: () => ({ runs: [], next: null }),
  },
}))

describe('the archived cards', () => {
  /* The date in the language the person chose, not the machine's: a runner
     under LC_ALL=en_US with the app in pt-BR still reads 20/01/2026. */
  it('dates an archived card in the chosen locale', async () => {
    setLocale('pt-BR')
    render(<ArchivedPane projectId="p1" onClose={vi.fn()} onChanged={vi.fn()} />)
    expect(await screen.findByText(/· arquivado em 20\/01\/2026/)).toBeTruthy()
  })

  it('dates an archived card in English when English is chosen', async () => {
    render(<ArchivedPane projectId="p1" onClose={vi.fn()} onChanged={vi.fn()} />)
    expect(await screen.findByText(/· archived 1\/20\/(20)?26/)).toBeTruthy()
  })

  it('brings one back', async () => {
    const onChanged = vi.fn()
    render(<ArchivedPane projectId="p1" onClose={vi.fn()} onChanged={onChanged} />)
    expect(await screen.findByText('Old idea')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Restore' }))
    await waitFor(() => expect(onChanged).toHaveBeenCalled())
    expect(restored).toHaveBeenCalledWith('card_a')
  })

  it('deletes one only after asking', async () => {
    deleted.mockClear()
    render(<ArchivedPane projectId="p1" onClose={vi.fn()} onChanged={vi.fn()} />)
    fireEvent.click(await screen.findByRole('button', { name: 'Delete…' }))
    expect(deleted).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }))
    await waitFor(() => expect(deleted).toHaveBeenCalledWith('card_a', false))
  })

  it('closes only the question on Escape, not the list under it', async () => {
    const onClose = vi.fn()
    render(<ArchivedPane projectId="p1" onClose={onClose} onChanged={vi.fn()} />)
    fireEvent.click(await screen.findByRole('button', { name: 'Delete…' }))
    fireEvent.keyDown(window, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByText('Delete this card?')).toBeNull())
    expect(onClose).not.toHaveBeenCalled()
  })
})

describe('undo after an archive', () => {
  it('restores the card it was offered for', async () => {
    restored.mockClear()
    const onRestored = vi.fn()
    const onGone = vi.fn()
    render(<UndoArchive projectId="p1" cardId="card_b" onRestored={onRestored} onGone={onGone} />)
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }))
    await waitFor(() => expect(onGone).toHaveBeenCalled())
    expect(restored).toHaveBeenCalledWith('card_b')
    expect(onRestored).toHaveBeenCalled()
  })

  it('goes by itself after a few seconds', () => {
    vi.useFakeTimers()
    const onGone = vi.fn()
    render(<UndoArchive projectId="p1" cardId="card_b" onRestored={vi.fn()} onGone={onGone} />)
    act(() => {
      vi.advanceTimersByTime(UNDO_MS - 1)
    })
    expect(onGone).not.toHaveBeenCalled()
    act(() => {
      vi.advanceTimersByTime(1)
    })
    expect(onGone).toHaveBeenCalled()
  })
})
