import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
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
const { RemoteMic } = await import('./RemoteMic')

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

describe('the Remote’s mic in Portuguese', () => {
  /* A phone that can record for the machine, whose permission answer the test picks. */
  const recordable = (allowed: boolean): void => {
    class Recorder {
      static isTypeSupported = (): boolean => false
      state = 'inactive'
      mimeType = ''
      ondataavailable: ((event: { data: Blob }) => void) | null = null
      onstop: (() => void) | null = null
      start(): void {
        this.state = 'recording'
      }
      stop(): void {
        this.state = 'inactive'
        this.onstop?.()
      }
    }
    vi.stubGlobal('MediaRecorder', Recorder)
    const stream = { getTracks: () => [] } as unknown as MediaStream
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { getUserMedia: () => (allowed ? Promise.resolve(stream) : Promise.reject(new Error('denied'))) },
    })
  }

  afterEach(() => {
    vi.unstubAllGlobals()
    Reflect.deleteProperty(navigator, 'mediaDevices')
  })

  it('says the microphone was refused, in the host’s language', async () => {
    setLocale('pt-BR')
    recordable(false)
    const refused = vi.fn()
    render(<RemoteMic machine onRecorded={() => {}} onHeard={() => {}} onRefused={refused} />)
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Ditar' })))
    expect(refused).toHaveBeenCalledWith('o microfone não foi permitido')
  })

  it('names the button for stopping while it records', async () => {
    setLocale('pt-BR')
    recordable(true)
    render(<RemoteMic machine onRecorded={() => {}} onHeard={() => {}} onRefused={() => {}} />)
    expect(screen.getByRole('button', { name: 'Ditar' }).getAttribute('title')).toBe('Ouvido pelo motor de voz da máquina')
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Ditar' })))
    expect(screen.getByRole('button', { name: 'Parar de ditar' }).textContent).toBe('Parar')
  })
})
