import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Paused } from '../gen/bindings'
import { setLocale } from '../i18n'
import { PauseControl } from './PauseControl'
import { pauseOptions } from './usePause'

const heard: Record<string, (payload: unknown) => void> = {}
vi.mock('./window', () => ({
  onCarried: (name: string, then: (payload: unknown) => void) => {
    heard[name] = then
    return () => delete heard[name]
  },
}))

let state: Paused = { on: false, until: null }
const set = vi.fn(async (until: number | null, forever: boolean): Promise<Paused> => {
  state = forever ? { on: true, until: null } : until === null ? { on: false, until: null } : { on: true, until }
  return state
})
vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => ({ data: await call(), error: null, loading: false }),
  commands: { pauseRead: async () => state, pauseSet: (until: number | null, forever: boolean) => set(until, forever) },
}))

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  state = { on: false, until: null }
  setLocale('en')
})

describe('pausing devpit', () => {
  it('pauses until resumed, says so in the strip, and resumes', async () => {
    render(<PauseControl />)
    fireEvent.click(screen.getByRole('button', { name: /Pause/ }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'Until I resume' }))
    await waitFor(() => expect(set).toHaveBeenCalledWith(null, true))
    await waitFor(() => screen.getByText('Paused until you resume · Resume'))
    fireEvent.click(screen.getByText('Paused until you resume · Resume'))
    await waitFor(() => expect(set).toHaveBeenLastCalledWith(null, false))
  })

  it('hears a pause given in another window', async () => {
    render(<PauseControl />)
    await act(async () => heard['pause:changed']!({ on: true, until: null }))
    expect(screen.getByText(/Paused until you resume/)).toBeTruthy()
  })

  it('offers half an hour, an hour, tomorrow morning and until resumed', () => {
    const now = new Date(2026, 9, 1, 18, 0).getTime() / 1000
    const [half, hour, morning, forever] = pauseOptions(now)
    expect(half!.until! - now).toBe(1800)
    expect(hour!.until! - now).toBe(3600)
    expect(new Date(morning!.until! * 1000).getHours()).toBe(9)
    expect(forever!.until).toBeNull()
  })

  it('says the whole pause in Portuguese, moment and all', async () => {
    setLocale('pt-BR')
    render(<PauseControl />)
    await act(async () => heard['pause:changed']!({ on: true, until: null }))
    expect(screen.getByText('Pausado até você retomar · Retomar')).toBeTruthy()
    const at = new Date(2026, 9, 1, 18, 0).getTime() / 1000 + 3600
    await act(async () => heard['pause:changed']!({ on: true, until: at }))
    const hour = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' }).format(new Date(at * 1000))
    expect(screen.getByText(`Pausado até ${hour} · Retomar`)).toBeTruthy()
  })
})
