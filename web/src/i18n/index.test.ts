import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { createElement } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { LocaleRoot, useLocale } from './LocaleRoot'
import { setLocale, t } from './index'

/* Only the field this file reads: the core resolves the choice (T3) and hands
   the web a locale, so the web never guesses from the browser. */
let chosen = 'en'

vi.mock('../shell/live', () => ({
  ask: async (call: () => Promise<{ data?: unknown }>) => ({
    data: ((await call()) as { data?: unknown } | undefined)?.data ?? null,
    error: null,
    loading: false,
  }),
  commands: {
    settingsRead: async () => ({ status: 'ok', data: { locale: chosen } }),
  },
}))

/* Says a translated sentence, and hears the locale so a change reaches it. */
function Says(): React.JSX.Element {
  useLocale()
  return createElement('p', null, t('greet.hello', { name: 'Ada', surname: 'Lovelace' }))
}

afterEach(() => {
  cleanup()
  setLocale('en')
  chosen = 'en'
  document.documentElement.lang = 'en'
  vi.unstubAllEnvs()
  vi.restoreAllMocks()
})

describe('a sentence with placeholders', () => {
  it('fills each {name} from the args', () => {
    setLocale('en')
    expect(t('greet.hello', { name: 'Ada' })).toBe('Hello, Ada')
  })

  it('picks one or other by the locale the sentence is in', () => {
    setLocale('en')
    expect(t('board.cards', { count: 1 })).toBe('1 card')
    expect(t('board.cards', { count: 3 })).toBe('3 cards')

    setLocale('pt-BR')
    expect(t('board.cards', { count: 1 })).toBe('1 cartão')
    expect(t('board.cards', { count: 3 })).toBe('3 cartões')
  })
})

describe('a key the locale does not carry', () => {
  it('falls back to the English text', () => {
    setLocale('pt-BR')
    expect(t('board.englishOnly')).toBe('Only the English catalog carries this one')
  })

  it('says the last key segment, readably, and warns in dev', () => {
    setLocale('pt-BR')
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    expect(t('board.emptyLane')).toBe('empty lane')
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('board.emptyLane'))
  })

  it('stays quiet outside dev', () => {
    setLocale('pt-BR')
    vi.stubEnv('DEV', false)
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    expect(t('board.emptyLane')).toBe('empty lane')
    expect(warn).not.toHaveBeenCalled()
  })
})

describe('the LocaleRoot', () => {
  it('stamps the document language from the settings the core resolved', async () => {
    chosen = 'pt-BR'
    render(createElement(LocaleRoot, null, createElement(Says)))
    await waitFor(() => expect(document.documentElement.lang).toBe('pt-BR'))
    expect(screen.getByText('Olá, Ada Lovelace')).toBeTruthy()
  })

  it('renders again when the choice changes, with no reload', async () => {
    chosen = 'en'
    render(createElement(LocaleRoot, null, createElement(Says)))
    await waitFor(() => expect(document.documentElement.lang).toBe('en'))
    expect(screen.getByText('Hello, Ada')).toBeTruthy()

    setLocale('pt-BR')
    await waitFor(() => expect(document.documentElement.lang).toBe('pt-BR'))
    expect(screen.getByText('Olá, Ada Lovelace')).toBeTruthy()
  })
})

describe('a placeholder one language carries and the other lacks', () => {
  it('fills it where it is, and drops it where it is not', () => {
    setLocale('pt-BR')
    expect(t('greet.hello', { name: 'Ada', surname: 'Lovelace' })).toBe('Olá, Ada Lovelace')

    setLocale('en')
    expect(t('greet.hello', { name: 'Ada', surname: 'Lovelace' })).toBe('Hello, Ada')
  })
})
