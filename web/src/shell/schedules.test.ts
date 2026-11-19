import { afterEach, describe, expect, it } from 'vitest'

import { setLocale } from '../i18n'
import { repeatSaid, stateSaid, whenSaid } from './schedules'

afterEach(() => setLocale('en'))

const routine = (days: number[]) => ({ kind: 'morning' as const, minute: 540, days, zone: 'UTC', offsetMinutes: 0 })

describe('what a Schedules row says', () => {
  it('says whether it repeats', () => {
    expect(repeatSaid(null)).toBe('Once')
    expect(repeatSaid(routine([]))).toBe('Every day')
    expect(repeatSaid(routine([5, 1, 2, 3, 4]))).toBe('Weekdays')
    expect(repeatSaid(routine([6, 0]))).toBe('Weekends')
    expect(repeatSaid(routine([1, 3]))).toBe('Mon, Wed')
  })

  it('says its state in words', () => {
    expect(stateSaid('due')).toBe('Went off')
    expect(stateSaid('paused')).toBe('Paused')
  })
})

describe('when a routine goes off', () => {
  /* Noon UTC is two different days on two different runners, so the month is
     what the test reads — the hour and the zone are the machine's. */
  const day = Date.UTC(2026, 0, 20, 12) / 1000

  it('says the date in the locale the person chose', () => {
    setLocale('en')
    expect(whenSaid(day)).toContain('Jan')
    setLocale('pt-BR')
    expect(whenSaid(day)).toContain('jan')
  })

  it('still says the date at a zone it does not know', () => {
    setLocale('en')
    expect(whenSaid(day, 'Mars/Olympus')).toContain('Jan')
  })

  it('says a dash when there is no date', () => {
    expect(whenSaid(null)).toBe('—')
  })
})
