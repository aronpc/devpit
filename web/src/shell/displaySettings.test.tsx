import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { LocaleRoot } from '../i18n/LocaleRoot'
import { setLocale } from '../i18n'
import type { Display, Settings as Stored } from '../gen/bindings'
import { DisplaySettings, LanguageSettings } from './DisplaySettings'

let display: Display = { offered: true, running: 'wayland', chosen: null, why: 'default' }
let stored: Stored = {
  theme: 'system',
  language: 'system',
  locale: 'en',
  onboardedAt: 0,
  automaticUpdates: null,
  confirmStop: null,
  terminalContrast: null,
  focusMode: null,
  errorReports: null,
  island: null,
  reminders: null,
}
const set = vi.fn(async (backend: Display['chosen']) => (display = { ...display, chosen: backend, why: 'chosen' }))
const write = vi.fn(async (language: Stored['language']) =>
  (stored = { ...stored, language, locale: language === 'pt_br' ? 'pt-BR' : 'en' }))
vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => ({ data: await call(), error: null, loading: false }),
  commands: {
    displayRead: async () => display,
    displaySet: (backend: Display['chosen']) => set(backend),
    settingsRead: async () => stored,
    settingsWrite: (_theme: unknown, language: Stored['language']) => write(language),
  },
}))
vi.mock('./window', () => ({ onEvent: () => () => {} }))

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  display = { offered: true, running: 'wayland', chosen: null, why: 'default' }
  stored = { ...stored, language: 'system', locale: 'en' }
  setLocale('en')
  document.documentElement.lang = 'en'
})

describe('the display setting', () => {
  it('says what this start runs on and why', async () => {
    display = { ...display, running: 'x11', why: 'failedBefore' }
    render(<DisplaySettings />)
    expect(await screen.findByText(/On X11 \(XWayland\)\. A Wayland start of this version never loaded its window/)).toBeTruthy()
    expect(screen.getByRole('radio', { name: 'Automatic' }).getAttribute('aria-checked')).toBe('true')
  })

  it('records the choice for the next start', async () => {
    render(<DisplaySettings />)
    fireEvent.click(await screen.findByRole('radio', { name: 'X11' }))
    await waitFor(() => expect(set).toHaveBeenCalledWith('x11'))
    await waitFor(() => expect(screen.getByRole('radio', { name: 'X11' }).getAttribute('aria-checked')).toBe('true'))
  })

  it('is not offered where there is no choice', async () => {
    display = { ...display, offered: false }
    const { container } = render(<DisplaySettings />)
    await waitFor(() => expect(container.innerHTML).toBe(''))
  })
})

describe('the language setting', () => {
  it('offers system, English and Português (Brasil), each in its own name', async () => {
    render(<LocaleRoot><LanguageSettings /></LocaleRoot>)
    expect(await screen.findByRole('radio', { name: 'System' })).toBeTruthy()
    expect(screen.getByRole('radio', { name: 'English' })).toBeTruthy()
    expect(screen.getByRole('radio', { name: 'Português (Brasil)' })).toBeTruthy()
  })

  it('writes the choice and the mounted row speaks the locale it answered', async () => {
    render(<LocaleRoot><LanguageSettings /></LocaleRoot>)
    fireEvent.click(await screen.findByRole('radio', { name: 'Português (Brasil)' }))
    await waitFor(() => expect(write).toHaveBeenCalledWith('pt_br'))
    expect(await screen.findByRole('radio', { name: 'Sistema' })).toBeTruthy()
    expect(screen.getByRole('radio', { name: 'English' })).toBeTruthy()
    expect(screen.getByRole('radio', { name: 'Português (Brasil)' }).getAttribute('aria-checked')).toBe('true')
  })

  it('speaks the display row in the locale Settings resolved', async () => {
    stored = { ...stored, language: 'pt_br', locale: 'pt-BR' }
    render(<LocaleRoot><DisplaySettings /></LocaleRoot>)
    expect(await screen.findByRole('radiogroup', { name: 'Tela' })).toBeTruthy()
    expect(screen.getByRole('radio', { name: 'Automático' })).toBeTruthy()
    expect(screen.getByText(/Wayland onde o desktop tem/)).toBeTruthy()
  })
})
