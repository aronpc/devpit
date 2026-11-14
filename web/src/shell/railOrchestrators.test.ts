import { describe, expect, it } from 'vitest'

import type { Project, Thread } from '../gen/bindings'
import { byAge, lastSpoken, onAccount } from './RailOrchestrators'

const made = (id: string): Project => ({ id }) as Project

describe('byAge', () => {
  it('keeps orchestrators where they were made, however the list arrives', () => {
    const lastOpenedFirst = [made('prj_01M3DT'), made('prj_01M3CT')]
    expect(byAge(lastOpenedFirst).map((one) => one.id)).toEqual(['prj_01M3CT', 'prj_01M3DT'])
    expect(byAge([...lastOpenedFirst].reverse()).map((one) => one.id)).toEqual(['prj_01M3CT', 'prj_01M3DT'])
  })
})

describe('onAccount', () => {
  const thread = (id: string, profile: string, lastAt: number): Thread => ({ id, title: id, profile, model: null, costUsd: 0, lastAt })

  it('reopens the last conversation on the account spoken as now, never one begun on the old', () => {
    const threads = [thread('old', 'claude', 30), thread('new', 'codex', 20)]
    expect(lastSpoken(onAccount(threads, 'codex'))?.id).toBe('new')
    expect(lastSpoken(onAccount(threads, 'claude'))?.id).toBe('old')
    expect(lastSpoken(onAccount(threads, 'opencode'))).toBeUndefined()
  })
})
