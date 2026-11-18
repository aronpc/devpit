import { describe, expect, it } from 'vitest'

import type { PlannedCard } from '../gen/bindings'
import { abuseSaid, closedLabel, cut, edited, linesOf, moved, named, stillBlocking } from './planEdit'

const card = (key: string, dependsOn: string[] = []): PlannedCard => ({
  key,
  title: `card ${key}`,
  build: '',
  criteria: [],
  files: [],
  verify: '',
  dependsOn,
})

const PLAN = [card('s1'), card('s2', ['s1']), card('s3', ['s1', 's2'])]
const keys = (cards: readonly PlannedCard[]): string[] => cards.map((one) => one.key)

describe('a plan on the board', () => {
  it('moves a card within the plan, and not past its ends', () => {
    expect(keys(moved(PLAN, 2, -1))).toEqual(['s1', 's3', 's2'])
    expect(keys(moved(PLAN, 0, 1))).toEqual(['s2', 's1', 's3'])
    expect(keys(moved(PLAN, 0, -1))).toEqual(['s1', 's2', 's3'])
    expect(keys(moved(PLAN, 2, 1))).toEqual(['s1', 's2', 's3'])
  })

  it('cuts a card and leaves nothing waiting on it', () => {
    const left = cut(PLAN, 's2')
    expect(keys(left)).toEqual(['s1', 's3'])
    expect(left[1]!.dependsOn).toEqual(['s1'])
  })

  it('edits one card and never its key', () => {
    const next = edited(PLAN, 's2', { title: 'renamed', key: 'other' })
    expect(next[1]).toEqual({ ...PLAN[1], title: 'renamed' })
    expect(next[0]).toBe(PLAN[0])
  })

  it("keeps a card's abuse cases and trust boundary through an edit", () => {
    const guarded = { ...card('s1'), abuse: ['a path outside the root is refused'], boundary: true }
    const next = edited([guarded], 's1', { title: 'renamed' })
    expect(next[0]).toEqual({ ...guarded, title: 'renamed' })
    expect(abuseSaid(guarded)).toBe('Crosses a trust boundary · Abuse cases: a path outside the root is refused')
    expect(abuseSaid({ boundary: true })).toBe('Crosses a trust boundary · No abuse cases yet')
  })

  it('reads criteria one per line', () => {
    expect(linesOf(' one \n\n two\n')).toEqual(['one', 'two'])
  })

  it('counts only what is neither done nor released as blocking', () => {
    expect(
      stillBlocking([
        { id: 'a', title: 'a', done: true },
        { id: 'b', title: 'b', done: false },
        { id: 'c', title: 'c', done: false, released: true },
      ]).map((one) => one.id),
    ).toEqual(['b'])
    expect(stillBlocking(undefined)).toEqual([])
  })

  it('says a card merged into another or cut from the plan', () => {
    expect(closedLabel({ supersededBy: { id: 'a', title: 'Index bodies', done: false, number: 2 } })).toBe('Merged into #2 Index bodies')
    expect(closedLabel({ cut: true })).toBe('Cut from the plan')
    expect(closedLabel({})).toBeNull()
  })
})

describe('named', () => {
  it('says a blocker by its number when it has one', () => {
    expect(named({ id: 'a', title: 'Index bodies', done: false, number: 42 })).toBe('#42 Index bodies')
    expect(named({ id: 'a', title: 'Index bodies', done: false, number: null })).toBe('Index bodies')
  })
})
