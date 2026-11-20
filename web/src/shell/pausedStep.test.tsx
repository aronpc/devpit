import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { Run, RunSwitchOptions } from '../gen/bindings'
import { PausedStep, RunIdentity } from './PausedStep'

afterEach(cleanup)

const paused: Run = {
  id: 'run_1',
  stepId: 'step_1',
  stepName: 'Review',
  state: 'paused',
  output: null,
  exitCode: null,
  costUsd: null,
  durationMs: null,
  startedAt: 0,
  profile: 'Old account',
  model: 'sonnet',
}

let options: RunSwitchOptions = {
  reason: "Old account's 5-hour quota is exhausted",
  profiles: [
    { id: 'work', label: 'Work account', model: 'opus' },
    { id: 'personal', label: 'Personal', model: null },
  ],
}
const switched = vi.fn()

vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: {
    runSwitchOptions: () => options,
    runSwitch: (_runId: string, profileId: string) => {
      switched(profileId)
      return { ...paused, id: 'run_2', state: 'running', profile: 'Work account', model: 'opus' }
    },
  },
}))

beforeEach(() => {
  options = {
    reason: "Old account's 5-hour quota is exhausted",
    profiles: [
      { id: 'work', label: 'Work account', model: 'opus' },
      { id: 'personal', label: 'Personal', model: null },
    ],
  }
  switched.mockClear()
})

describe('a paused step', () => {
  it('shows why it paused and only the profiles the command offered', async () => {
    render(<PausedStep run={paused} onSwitched={vi.fn()} />)

    expect(await screen.findByText(options.reason)).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Switch to Work account' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Switch to Personal' })).toBeTruthy()
    expect(screen.queryByText(/switched off/i)).toBeNull()
  })

  it('switches in one click and tells the run list to reload', async () => {
    const onSwitched = vi.fn()
    render(<PausedStep run={paused} onSwitched={onSwitched} />)

    fireEvent.click(await screen.findByRole('button', { name: 'Switch to Work account' }))

    await waitFor(() => expect(switched).toHaveBeenCalledWith('work'))
    expect(onSwitched).toHaveBeenCalled()
  })

  it('says when no profile can take over and leaves no dead button', async () => {
    options = { reason: 'the profile no longer exists', profiles: [] }
    render(<PausedStep run={paused} onSwitched={vi.fn()} />)

    expect(await screen.findByText('No other profile can run this step.')).toBeTruthy()
    expect(screen.queryByRole('button')).toBeNull()
  })

  it('names the account and model on a run', () => {
    render(<RunIdentity run={{ ...paused, profile: 'Work account', model: 'opus' }} />)
    expect(screen.getByText('Work account · opus')).toBeTruthy()
  })
})
