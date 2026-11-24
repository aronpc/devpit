import { afterEach, describe, expect, it } from 'vitest'

import type { Commit } from '../gen/bindings'
import { setLocale } from '../i18n'
import { appended, byDay, dayOf, matches, since, subjectOf } from './commitLog'

const commit = (sha: string, committedAt: number | null, subject = 'fix: x'): Commit => ({ sha, subject, author: 'Jhol', committedAt })

describe('the history tab', () => {
  it('reads a conventional subject, and leaves any other as written', () => {
    expect(subjectOf('feat(rail)!: drags work')).toEqual({ kind: 'feat', scope: 'rail', breaking: true, text: 'drags work' })
    expect(subjectOf('chore: 0.1.11')).toEqual({ kind: 'chore', scope: null, breaking: false, text: '0.1.11' })
    expect(subjectOf('Merge branch main')).toEqual({ kind: null, scope: null, breaking: false, text: 'Merge branch main' })
  })

  it('files commits under today, yesterday, then dates', () => {
    const now = new Date(2026, 8, 23, 10, 0)
    const at = (d: Date): number => d.getTime() / 1000
    expect(dayOf(at(new Date(2026, 8, 23, 1, 0)), now)).toBe('Today')
    expect(dayOf(at(new Date(2026, 8, 22, 23, 0)), now)).toBe('Yesterday')
    expect(dayOf(null, now)).toBe('Undated')
    const days = byDay([commit('a', at(new Date(2026, 8, 23, 9))), commit('b', at(new Date(2026, 8, 23, 8))), commit('c', at(new Date(2026, 8, 22, 8)))], now)
    expect(days.map((one) => [one.day, one.commits.length])).toEqual([['Today', 2], ['Yesterday', 1]])
  })

  it('never shows a commit twice when an older page overlaps the one before', () => {
    expect(appended([commit('a', 1), commit('b', 1)], [commit('b', 1), commit('c', 1)]).map((one) => one.sha)).toEqual(['a', 'b', 'c'])
  })

  it('filters by subject, author or sha', () => {
    expect(matches(commit('44bafb5', 1, 'chore: 0.1.11'), '0.1.11')).toBe(true)
    expect(matches(commit('44bafb5', 1), '44baf')).toBe(true)
    expect(matches(commit('44bafb5', 1), 'jho')).toBe(true)
    expect(matches(commit('44bafb5', 1), 'nope')).toBe(false)
  })

  it('says how long ago in a few letters', () => {
    const now = 1_000_000_000_000
    expect(since(now / 1000 - 30, now)).toBe('now')
    expect(since(now / 1000 - 3 * 3600, now)).toBe('3h')
    expect(since(now / 1000 - 2 * 86400, now)).toBe('2d')
    expect(since(null, now)).toBe('')
  })
})

/* A weekday and a date in the language the person chose, not the machine's. */
describe("a commit's day in the chosen language", () => {
  const now = new Date(2026, 8, 23, 9, 0)
  const at = (day: Date): number => day.getTime() / 1000
  afterEach(() => setLocale('en'))

  it('reads in pt-BR when pt-BR is chosen', () => {
    setLocale('pt-BR')
    expect(dayOf(at(new Date(2026, 8, 20, 12)), now)).toBe('domingo')
    expect(dayOf(at(new Date(2026, 8, 1, 12)), now)).toMatch(/^1 de set\.?$/)
  })

  it('says today, yesterday and no date in pt-BR too', () => {
    setLocale('pt-BR')
    expect(dayOf(at(new Date(2026, 8, 23, 1, 0)), now)).toBe('Hoje')
    expect(dayOf(at(new Date(2026, 8, 22, 23, 0)), now)).toBe('Ontem')
    expect(dayOf(null, now)).toBe('Sem data')
  })

  it('reads in English when English is chosen', () => {
    expect(dayOf(at(new Date(2026, 8, 20, 12)), now)).toBe('Sunday')
    expect(dayOf(at(new Date(2026, 8, 1, 12)), now)).toBe('Sep 1')
  })
})
