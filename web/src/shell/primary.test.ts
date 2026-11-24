import { describe, expect, it } from 'vitest'

import type { Change } from '../gen/bindings'
import { counted, primary } from './primary'

const change = (over: Partial<Change> = {}): Change => ({
  path: 'a.rs',
  status: 'modified',
  added: 1,
  removed: 0,
  staged: false,
  ...over,
})

const many = (count: number, staged: boolean): Change[] =>
  Array.from({ length: count }, (_, at) => change({ path: `f${at}.rs`, staged }))

describe('the one button at the top', () => {
  it('offers to stage while anything is still loose', () => {
    // The step in front of you. A commit that silently left out the file you
    // just edited is the mistake this ordering prevents.
    const said = primary(many(3, false), '')
    expect(said.doing).toBe('stage')
    expect(said.label).toBe('Stage All')
    expect(said.disabled).toBe(false)
  })

  it('offers to stage even when a message is already written', () => {
    expect(primary(many(3, false), 'already typed').doing).toBe('stage')
  })

  it('turns into a commit once everything is staged', () => {
    const said = primary(many(2, true), 'why it changed')
    expect(said.doing).toBe('commit')
    expect(said.label).toBe('Commit 2')
    expect(said.disabled).toBe(false)
  })

  it('names how many files the commit would take', () => {
    expect(primary([...many(2, true), ...many(0, false)], 'x').label).toBe('Commit 2')
  })

  it('refuses a commit with nothing said, and says why', () => {
    // A disabled button that does not explain itself is a dead end.
    const said = primary(many(1, true), '   ')
    expect(said.disabled).toBe(true)
    expect(said.why).toBe('Say what changed, and why.')
  })

  it('has nothing to do with a clean tree, and says so', () => {
    const said = primary([], 'anything')
    expect(said.doing).toBe('nothing')
    expect(said.disabled).toBe(true)
    expect(said.why).toBe('Nothing has changed.')
  })

  it('says nothing when the button works', () => {
    expect(primary(many(1, true), 'a message').why).toBeNull()
  })

  it('counts only what is loose when it offers to stage', () => {
    expect(primary(many(3, false), '').files).toBe(3)
  })

  it('commits the part that is staged, leaving the rest alone', () => {
    // Staging a few files on purpose and committing only those is ordinary.
    // What stops a file being left out by accident is the CHANGED count in
    // the row under the button, not a button that refuses.
    const mixed = [
      ...many(2, true),
      ...Array.from({ length: 3 }, (_, at) => change({ path: `loose${at}.rs` })),
    ]
    const said = primary(mixed, 'partial, on purpose')
    expect(said.doing).toBe('commit')
    expect(said.label).toBe('Commit 2')
    expect(said.disabled).toBe(false)
  })
})

describe('line counts', () => {
  /* Against the runtime's own formatting and not against "6,624" — but with
     the locale said out loud on both sides, because one side taking the
     machine's would make the test mean something else from one runner to
     the next. */
  it('groups thousands, so six thousand does not read as six hundred', () => {
    expect(counted(6624, 'en')).toBe((6624).toLocaleString('en'))
    expect(counted(6624, 'en').length).toBeGreaterThan('6624'.length)
    expect(counted(241, 'en')).toBe((241).toLocaleString('en'))
    expect(counted(0, 'en')).toBe('0')
  })
})
