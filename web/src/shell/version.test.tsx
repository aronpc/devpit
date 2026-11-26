import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { AppInfo, UpdateStatus } from '../gen/bindings'
import { SHOW_THE_UPDATE } from './UpdateCard'
import { Version } from './Version'

const heard: Record<string, (payload: UpdateStatus) => void> = {}
vi.mock('./window', () => ({
  onCarried: (name: string, then: (payload: UpdateStatus) => void) => {
    heard[name] = then
    return () => delete heard[name]
  },
}))

let info: AppInfo = { version: '0.1.30', platform: 'linux', statePath: '/h/.devpit', dev: false }
const standing = vi.fn(async (): Promise<UpdateStatus> => ({ type: 'idle' }))
vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => ({ data: await call(), error: null, loading: false }),
  commands: { appInfo: async () => info, updateStatus: () => standing() },
}))

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  info = { version: '0.1.30', platform: 'linux', statePath: '/h/.devpit', dev: false }
})

describe('the version in the status strip', () => {
  it('says which devpit this is, from the app', async () => {
    render(<Version />)
    await waitFor(() => screen.getByRole('button', { name: /v0\.1\.30/ }))
    expect(document.querySelector('.strip__up')).toBeNull()
  })

  it('says when it is a build being worked on', async () => {
    info = { ...info, dev: true }
    render(<Version />)
    await waitFor(() => screen.getByText('v0.1.30 · dev'))
  })

  it('copies the version and the platform, for a bug report', async () => {
    const writeText = vi.fn(async () => undefined)
    Object.assign(navigator, { clipboard: { writeText } })
    render(<Version />)
    fireEvent.click(await waitFor(() => screen.getByRole('button', { name: /v0\.1\.30/ })))
    expect(writeText).toHaveBeenCalledWith('devpit 0.1.30 · linux')
    await waitFor(() => screen.getByText('Copied'))
  })

  it('marks a newer one and brings the update card forward', async () => {
    const shown = vi.fn()
    window.addEventListener(SHOW_THE_UPDATE, shown)
    standing.mockResolvedValueOnce({ type: 'available', version: '0.1.31', notes: '', kind: 'appImage', testFeed: false })
    render(<Version />)
    const button = await waitFor(() => screen.getByTitle(/0\.1\.31 is waiting/))
    expect(document.querySelector('.strip__up')).not.toBeNull()
    fireEvent.click(button)
    expect(shown).toHaveBeenCalledTimes(1)
    window.removeEventListener(SHOW_THE_UPDATE, shown)
  })

  it('does not mark an offer from a test feed', async () => {
    render(<Version />)
    await waitFor(() => screen.getByRole('button', { name: /v0\.1\.30/ }))
    act(() => heard['update:status']!({ type: 'available', version: '9.9.9', notes: '', kind: 'appImage', testFeed: true }))
    expect(document.querySelector('.strip__up')).toBeNull()
  })
})
