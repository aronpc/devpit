import { describe, expect, it } from 'vitest'

import type { WouldRun } from '../gen/bindings'
import { timeoutWords, wouldRunWords } from './wouldRun'

const would = (over: Partial<WouldRun> = {}): WouldRun => ({
  stepId: 'step_1',
  stepName: 'tests',
  irreversible: false,
  command: 'pnpm test',
  inDirectory: '/w/devpit',
  inAWorktree: false,
  declaredEnv: ['DEVPIT_BRANCH'],
  timeoutSeconds: 600,
  profile: null,
  model: null,
  check: null,
  ...over,
})

describe('what a check would do, in words', () => {
  /* Sabotage: say "No timeout" and the screen tells somebody a command will
     stop when nothing will stop it. */
  it('does not call the absence of a limit a decision to have none', () => {
    const said = timeoutWords(null)
    expect(said).toMatch(/nothing set a limit/i)
    expect(said).toMatch(/until it ends or you stop it/i)
  })

  it('says a limit in the unit a person reads it in', () => {
    expect(timeoutWords(30)).toBe('30 seconds.')
    expect(timeoutWords(60)).toBe('1 minute.')
    expect(timeoutWords(600)).toBe('10 minutes.')
  })

  /* A command step with no checkout runs in the project. One with a checkout
     gets a folder that may not exist yet, and saying "nowhere" would be wrong
     where "not made yet" is right. */
  it('tells a checkout that is not made yet from no directory at all', () => {
    expect(wouldRunWords(would({ inAWorktree: true })).noDirectory).toMatch(/made the first time/i)
    expect(wouldRunWords(would({ inAWorktree: false })).noDirectory).toMatch(/project folder/i)
  })

  it('says a step with no command line is one, not a command that does nothing', () => {
    expect(wouldRunWords(would({ command: null })).noCommand).toMatch(/agent turn or a shell/i)
  })

  it('says out loud that a step with no undo is asked about twice', () => {
    expect(wouldRunWords(would({ irreversible: true })).irreversible).toMatch(/no undo/i)
    expect(wouldRunWords(would({ irreversible: true })).irreversible).toMatch(/asks twice/i)
  })

  it('names the profile, requested model and check result before the step runs', () => {
    const said = wouldRunWords(
      would({ profile: 'Work account', model: 'sonnet', check: 'Ready' }),
    )
    expect(said.identity).toBe('Work account · sonnet')
    expect(said.check).toBe('Ready')
  })
})
