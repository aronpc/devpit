import { afterEach, describe, expect, it } from 'vitest'

import { setLocale } from './index'
import { formatDate, formatNumber, formatRelative } from './format'

/* The words themselves, and not the runtime's formatting of nothing: the
   machine's locale is whatever the runner happens to carry, and the choice
   is the person's. Said out loud on both sides, so a formatter that took
   the machine's fails on one runner or the other. */

afterEach(() => setLocale('en'))

describe('a screen saying thousands', () => {
  it('groups them in the locale the person chose', () => {
    setLocale('pt-BR')
    expect(formatNumber(6624)).toBe('6.624')
    setLocale('en')
    expect(formatNumber(6624)).toBe('6,624')
  })
})

describe('a screen saying a date', () => {
  /* Month and year alone: a day at noon UTC is two different days on two
     different runners, and which month it is not. */
  const day = new Date(Date.UTC(2026, 0, 20, 12))

  it('names the month in the locale the person chose', () => {
    setLocale('pt-BR')
    expect(formatDate(day, { month: 'long', year: 'numeric' })).toBe('janeiro de 2026')
    setLocale('en')
    expect(formatDate(day, { month: 'long', year: 'numeric' })).toBe('January 2026')
  })
})

describe('a screen saying how long ago', () => {
  it('says it in the locale the person chose', () => {
    setLocale('pt-BR')
    expect(formatRelative(-2, 'minute')).toBe('há 2 minutos')
    setLocale('en')
    expect(formatRelative(-2, 'minute')).toBe('2 minutes ago')
  })

  it('carries the options the caller gives', () => {
    setLocale('en')
    expect(formatRelative(-1, 'day', { numeric: 'auto' })).toBe('yesterday')
  })

  it('says a locale named out loud over the chosen one', () => {
    /* A rule with its own words to say still names one, as `since` does. */
    setLocale('pt-BR')
    expect(formatRelative(-2, 'minute', {}, 'en')).toBe('2 minutes ago')
  })
})
