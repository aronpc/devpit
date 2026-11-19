import { afterEach, describe, expect, it } from 'vitest'

import { setLocale } from '../i18n'

import { DAY_REMINDS_AT, dueLabel, fromField, nearness, toField, toTimeField } from './due'

/* A fixed clock at noon, so nothing here depends on when it runs. */
const NOON = new Date(2026, 4, 20, 12, 0, 0).getTime() / 1000
const at = (year: number, month: number, day: number, hour = 12): number =>
  new Date(year, month - 1, day, hour).getTime() / 1000

describe('how near a deadline is', () => {
  it('is nothing at all for a card without one', () => {
    expect(nearness(null, NOON)).toBeNull()
    expect(dueLabel(null, NOON)).toBe('')
  })

  /* The bug this exists to prevent: a card due at 09:00 turning red at 09:01.
     It is due *today* until the day ends, not from the minute it passes. */
  it('counts days, not hours', () => {
    expect(nearness(at(2026, 5, 20, 9), NOON)).toBe('today')
    expect(nearness(at(2026, 5, 20, 23), NOON)).toBe('today')
    expect(nearness(at(2026, 5, 20, 0), NOON)).toBe('today')
  })

  it('separates past, this week, and further out', () => {
    expect(nearness(at(2026, 5, 19), NOON)).toBe('past')
    expect(nearness(at(2026, 5, 21), NOON)).toBe('soon')
    expect(nearness(at(2026, 5, 27), NOON)).toBe('soon')
    expect(nearness(at(2026, 5, 28), NOON)).toBe('later')
  })

  it('says today in words and everything else as a date', () => {
    expect(dueLabel(at(2026, 5, 20), NOON)).toBe('today')
    expect(dueLabel(at(2026, 5, 27), NOON)).toMatch(/27/)
    /* The year appears only when it is not this one — otherwise every card
       carries four digits that never change. */
    expect(dueLabel(at(2026, 12, 1), NOON)).not.toMatch(/2026/)
    expect(dueLabel(at(2027, 1, 4), NOON)).toMatch(/2027/)
  })
})

describe('the date field', () => {
  it('is empty for a card with no deadline', () => {
    expect(toField(null)).toBe('')
    expect(fromField('')).toBeNull()
    expect(fromField('not a date')).toBeNull()
  })

  /* `new Date('2026-05-20')` parses as UTC, which is the 19th for anyone west
     of Greenwich. Round-tripping is what proves this one is built from parts. */
  it('gives back the day that was chosen, in the reader’s own zone', () => {
    const chosen = fromField('2026-05-20')
    expect(chosen).not.toBeNull()
    expect(toField(chosen)).toBe('2026-05-20')
    expect(new Date(chosen! * 1000).getDate()).toBe(20)
  })

  it('survives a round trip for every day of a month', () => {
    for (let day = 1; day <= 28; day++) {
      const text = `2026-03-${String(day).padStart(2, '0')}`
      expect(toField(fromField(text))).toBe(text)
    }
  })

  /* Stored at nine rather than midnight: the hour a day's reminder goes off,
     and a date at 00:00 local read back across a daylight-saving boundary
     lands on the day before. */
  it('stores a day alone at the hour it reminds, away from midnight', () => {
    const chosen = fromField('2026-05-20')!
    expect(new Date(chosen * 1000).getHours()).toBe(DAY_REMINDS_AT)
  })

  it('keeps a time when one is chosen, and gives it back', () => {
    const chosen = fromField('2026-05-20', '15:30')!
    const at = new Date(chosen * 1000)
    expect([at.getDate(), at.getHours(), at.getMinutes()]).toEqual([20, 15, 30])
    expect(toTimeField(chosen, true)).toBe('15:30')
    expect(toTimeField(chosen, false)).toBe('')
    expect(fromField('2026-05-20', 'soon')).toBe(fromField('2026-05-20'))
  })

  it('says the time beside the day when there is one', () => {
    const today = fromField('2026-05-20', '15:30')!
    expect(dueLabel(today, NOON, true)).toMatch(/^today \d/)
    expect(dueLabel(today, NOON)).toBe('today')
    expect(dueLabel(at(2026, 5, 23, 15), NOON, true)).toMatch(/ · /)
  })
})

/* The date in the language the person chose, not the machine's: a runner
   under LC_ALL=en_US with the app in pt-BR still reads the day first and a
   24-hour clock. */
describe('a deadline in the chosen language', () => {
  afterEach(() => setLocale('en'))

  it('reads in pt-BR when pt-BR is chosen', () => {
    setLocale('pt-BR')
    expect(dueLabel(at(2026, 5, 27, 15), NOON, true)).toMatch(/^27 de mai\.? · 15:00$/)
    expect(dueLabel(at(2027, 1, 4), NOON)).toMatch(/^4 de jan\.? de 2027$/)
  })

  it('reads in English when English is chosen', () => {
    expect(dueLabel(at(2026, 5, 27, 15), NOON, true)).toMatch(/^May 27 · 03:00\sPM$/)
    expect(dueLabel(at(2027, 1, 4), NOON)).toBe('Jan 4, 2027')
  })
})
