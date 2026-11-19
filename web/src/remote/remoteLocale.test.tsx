import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { RemoteOut } from '../gen/bindings'
import { setLocale } from '../i18n'
import type { Link } from './socket'

/* The Remote's words come from the host's welcome, not from this device. */
let heard: (message: RemoteOut) => void = () => {}
vi.mock('./socket', async (original) => ({
  ...(await original<typeof import('./socket')>()),
  tokenKept: () => 'token',
  connect: (_token: string, hear: (message: RemoteOut) => void, state: (up: boolean) => void): Link => {
    heard = hear
    state(true)
    return { send: () => {}, sendBytes: () => {}, close: () => {} } as unknown as Link
  },
}))

const { RemoteApp } = await import('./RemoteApp')

const welcome = (locale?: string): RemoteOut =>
  ({ t: 'welcome', device: 'phone', host: 'desk', typing: false, answering: false, binary: true, stops: true, ...(locale ? { locale } : {}) }) as RemoteOut

afterEach(() => {
  cleanup()
  setLocale('en')
})

describe('the Remote speaks the host’s language', () => {
  it('says its tabs in the locale of the welcome, whatever this device speaks', () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('en-US')
    render(<RemoteApp />)
    act(() => heard(welcome('pt-BR')))
    expect(screen.getByRole('button', { name: 'terminais' })).toBeTruthy()
    expect(document.documentElement.lang).toBe('pt-BR')
  })

  it('follows the next welcome when the host changed its language', () => {
    render(<RemoteApp />)
    act(() => heard(welcome('pt-BR')))
    act(() => heard(welcome('en')))
    expect(screen.getByRole('button', { name: 'terminals' })).toBeTruthy()
  })

  it('stays in English for an older host whose welcome says none', () => {
    render(<RemoteApp />)
    act(() => heard(welcome()))
    expect(screen.getByRole('button', { name: 'terminals' })).toBeTruthy()
  })
})
