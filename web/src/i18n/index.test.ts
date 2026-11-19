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
  return createElement('p', null, t('greet.hello', { name: 'Ada' }))
}

/* The real catalogs stay parallel — that parity is a guard (T7), so no test
   data may bend them. What needs a bent pair (a key English alone carries, a
   placeholder one language fills and the other drops) is proven on catalogs
   made for the test: the module is loaded again over these. */
async function overCatalogs(
  english: Record<string, unknown>,
  portuguese: Record<string, unknown>,
): Promise<typeof import('./index')> {
  vi.resetModules()
  vi.doMock('./en/common.json', () => ({ default: english }))
  vi.doMock('./pt-BR/common.json', () => ({ default: portuguese }))
  return import('./index')
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
  it('falls back to the English text', async () => {
    const over = await overCatalogs(
      { 'board.englishOnly': 'Only the English catalog carries this one' },
      {},
    )
    over.setLocale('pt-BR')
    expect(over.t('board.englishOnly')).toBe('Only the English catalog carries this one')
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
    expect(screen.getByText('Olá, Ada')).toBeTruthy()
  })

  it('renders again when the choice changes, with no reload', async () => {
    chosen = 'en'
    render(createElement(LocaleRoot, null, createElement(Says)))
    await waitFor(() => expect(document.documentElement.lang).toBe('en'))
    expect(screen.getByText('Hello, Ada')).toBeTruthy()

    setLocale('pt-BR')
    await waitFor(() => expect(document.documentElement.lang).toBe('pt-BR'))
    expect(screen.getByText('Olá, Ada')).toBeTruthy()
  })
})

describe('a placeholder one language carries and the other lacks', () => {
  it('fills it where it is, and drops it where it is not', async () => {
    const over = await overCatalogs(
      { 'greet.hello': 'Hello, {name}' },
      { 'greet.hello': 'Olá, {name} {surname}' },
    )
    over.setLocale('pt-BR')
    expect(over.t('greet.hello', { name: 'Ada', surname: 'Lovelace' })).toBe('Olá, Ada Lovelace')

    over.setLocale('en')
    expect(over.t('greet.hello', { name: 'Ada', surname: 'Lovelace' })).toBe('Hello, Ada')
  })
})
