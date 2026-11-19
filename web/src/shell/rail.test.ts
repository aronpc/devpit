import { afterEach, describe, expect, it } from 'vitest'

import type { Project } from '../gen/bindings'
import {
  hueOf,
  initials,
  moved,
  orchestratorsInOrder,
  ordered,
  placed,
  savedOrchestrators,
  savedOrder,
  saveOrchestrators,
  sections,
  shown,
} from './rail'

const project = (id: string, name = id, group: string | null = null): Project =>
  ({ id, name, group, rootPath: `/w/${name}` }) as unknown as Project

describe('the rail', () => {
  it('keeps the order the person gave it, whatever was opened last', () => {
    const byClock = [project('b'), project('a'), project('c')]
    expect(ordered(byClock, ['a', 'b', 'c']).map((one) => one.id)).toEqual(['a', 'b', 'c'])
  })

  it('puts a project nobody placed yet at the end, by name', () => {
    const list = [project('z', 'zeta'), project('x', 'alpha'), project('a')]
    expect(ordered(list, ['a']).map((one) => one.id)).toEqual(['a', 'x', 'z'])
  })

  it('moves an icon to where another one is, in either direction', () => {
    expect(moved(['a', 'b', 'c', 'd'], 'a', 'c')).toEqual(['b', 'c', 'a', 'd'])
    expect(moved(['a', 'b', 'c', 'd'], 'd', 'b')).toEqual(['a', 'd', 'b', 'c'])
    expect(moved(['a', 'b'], 'a', 'a')).toEqual(['a', 'b'])
  })

  it('names a project by one or two letters', () => {
    expect(initials('devpit')).toBe('DE')
    expect(initials('devpit-app')).toBe('DA')
    expect(initials('my project')).toBe('MP')
    expect(initials('x')).toBe('X')
  })

  it('gives each project a hue of its own that does not change between runs', () => {
    expect(hueOf('prj_01M2S1D4YD9KYTAZCGECKP22DP')).toBe(hueOf('prj_01M2S1D4YD9KYTAZCGECKP22DP'))
    expect(hueOf('prj_a')).not.toBe(hueOf('prj_b'))
    expect(hueOf('prj_a')).toBeGreaterThanOrEqual(0)
    expect(hueOf('prj_a')).toBeLessThan(360)
  })
})

describe('groups in the rail', () => {
  it('puts the ungrouped first, then each group where its first project stands', () => {
    const list = [project('a', 'a', 'Work'), project('b'), project('c', 'c', 'Home'), project('d', 'd', 'Work')]
    expect(sections(list).map((one) => [one.group, one.projects.map((p) => p.id)])).toEqual([
      [null, ['b']],
      ['Work', ['a', 'd']],
      ['Home', ['c']],
    ])
  })

  it('has no empty section when every project is in a group', () => {
    expect(sections([project('a', 'a', 'Work')]).map((one) => one.group)).toEqual(['Work'])
  })
})

describe('the order of groups', () => {
  const list = [project('a', 'a', 'Work'), project('b', 'b', 'Home'), project('c')]

  it('follows the order the person gave the groups, ungrouped first', () => {
    expect(sections(list, ['Home', 'Work']).map((one) => one.group)).toEqual([null, 'Home', 'Work'])
  })

  it('puts a group nobody placed where its first project stands, after the placed ones', () => {
    expect(sections(list, ['Home']).map((one) => one.group)).toEqual([null, 'Home', 'Work'])
    expect(sections(list, []).map((one) => one.group)).toEqual([null, 'Work', 'Home'])
  })

  it('shows only the active of a filtered group, and always the one in front', () => {
    const [work] = sections([project('a', 'a', 'Work'), project('b', 'b', 'Work'), project('c', 'c', 'Work')])
    const only = { active: (id: string) => id === 'a', current: 'c' }
    expect(shown(work!, false, only).map((one) => one.id)).toEqual(['a', 'c'])
    expect(shown(work!, true, only)).toEqual([])
  })

  it('shows nothing of a folded group', () => {
    const [work] = sections([project('a', 'a', 'Work')])
    expect(shown(work!, true)).toEqual([])
    expect(shown(work!, false).map((one) => one.id)).toEqual(['a'])
  })
})

describe('a drop beside a row', () => {
  it('lands before or after it, from either direction', () => {
    expect(placed(['a', 'b', 'c', 'd'], 'a', 'c', false)).toEqual(['b', 'a', 'c', 'd'])
    expect(placed(['a', 'b', 'c', 'd'], 'a', 'c', true)).toEqual(['b', 'c', 'a', 'd'])
    expect(placed(['a', 'b', 'c', 'd'], 'd', 'a', false)).toEqual(['d', 'a', 'b', 'c'])
    expect(placed(['a', 'b'], 'a', 'a', true)).toEqual(['a', 'b'])
  })
})

describe('the order of orchestrators', () => {
  afterEach(() => localStorage.clear())

  const ids = (list: readonly Project[]): readonly string[] => list.map((one) => one.id)

  it('orchestrators order: keeps the saved order whatever was opened last', () => {
    const lastOpenedFirst = [project('prj_01M3C'), project('prj_01M3A'), project('prj_01M3B')]
    const order = ['prj_01M3B', 'prj_01M3C', 'prj_01M3A']
    expect(ids(orchestratorsInOrder(lastOpenedFirst, order))).toEqual(order)
    expect(ids(orchestratorsInOrder([...lastOpenedFirst].reverse(), order))).toEqual(order)
  })

  it('orchestrators order: puts a new one at the end by age, not by name', () => {
    const list = [project('prj_01M3Z', 'alpha'), project('prj_01M3P'), project('prj_01M3Y', 'zeta')]
    expect(ids(orchestratorsInOrder(list, ['prj_01M3P']))).toEqual(['prj_01M3P', 'prj_01M3Y', 'prj_01M3Z'])
  })

  it('orchestrators order: ignores a saved id that is gone', () => {
    const list = [project('prj_01M3A'), project('prj_01M3C')]
    const order = ['prj_01M3C', 'prj_01M3GONE', 'prj_01M3A']
    expect(ids(orchestratorsInOrder(list, order))).toEqual(['prj_01M3C', 'prj_01M3A'])
  })

  it('orchestrators order: round-trips through its own storage key', () => {
    saveOrchestrators(['prj_01M3B', 'prj_01M3A'])
    expect(savedOrchestrators()).toEqual(['prj_01M3B', 'prj_01M3A'])
    expect(JSON.parse(localStorage.getItem('devpit.rail.orchestrators')!)).toEqual(['prj_01M3B', 'prj_01M3A'])
    expect(localStorage.getItem('devpit.rail.order')).toBeNull()
    expect(savedOrder()).toEqual([])
  })
})
