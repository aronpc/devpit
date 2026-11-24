import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { IslandAsked, RemoteDraft, RemoteIn, RemoteQuestion, RemoteStop } from '../gen/bindings'
import { RemoteWaiting } from './RemoteWaiting'

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

const stop = (profile: string, name: string, losing: string | null = null): RemoteStop => ({ profile, stop: { name, pid: null, losing } })
const draft = (name: string, text: string): RemoteDraft => ({ profile: 'claude', name, text })

function shown(stops: RemoteStop[], drafts: RemoteDraft[], typing = true) {
  const send = vi.fn<(message: RemoteIn) => void>()
  vi.stubGlobal('confirm', vi.fn(() => true))
  render(<RemoteWaiting questions={[]} drafts={drafts} stops={stops} health={null} answering={false} typing={typing} send={send} />)
  return send
}

describe('what waits on the person, on the Remote', () => {
  it('lists the stops asked and stops them all on one press, as shown', () => {
    const stops = [stop('claude', 'api'), stop('claude', 'web', 'it is working right now'), stop('work', 'docs')]
    const send = shown(stops, [])
    expect(screen.getByText('api')).toBeTruthy()
    expect(screen.getByText('it is working right now')).toBeTruthy()
    expect(send).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Stop all 3' }))
    expect(send).toHaveBeenCalledTimes(1)
    expect(send).toHaveBeenCalledWith({ t: 'stopsConfirm', stops })
  })

  it('keeps them running without stopping any', () => {
    const stops = [stop('claude', 'api')]
    const send = shown(stops, [])
    fireEvent.click(screen.getByRole('button', { name: 'Keep running' }))
    expect(send).toHaveBeenCalledWith({ t: 'stopsDrop', stops })
    expect(send).not.toHaveBeenCalledWith(expect.objectContaining({ t: 'stopsConfirm' }))
  })

  it('sends every draft on one press', () => {
    const send = shown([], [draft('api', 'go on'), draft('web', 'run the tests')])
    fireEvent.click(screen.getByRole('button', { name: 'Send all 2' }))
    expect(send.mock.calls.map(([one]) => one)).toEqual([
      { t: 'draftSend', profile: 'claude', name: 'api' },
      { t: 'draftSend', profile: 'claude', name: 'web' },
    ])
  })

  it('offers no press to a device that may only watch', () => {
    shown([stop('claude', 'api')], [draft('api', 'go on'), draft('web', 'run the tests')], false)
    expect(screen.queryByRole('button', { name: /Stop|Send/ })).toBeNull()
  })
})

const asked = (question: string, options: string[], multi = false): IslandAsked => ({ header: null, question, options, multi })
const question = (id: string, tool: string, questions: IslandAsked[] | null, from = 'chat'): RemoteQuestion => ({
  id,
  from,
  project: null,
  tool,
  input: '{}',
  touch: questions ? { kind: 'ask', questions } : null,
})

function asking(questions: RemoteQuestion[]) {
  const send = vi.fn<(message: RemoteIn) => void>()
  vi.stubGlobal('confirm', vi.fn(() => true))
  render(<RemoteWaiting questions={questions} drafts={[]} stops={[]} health={null} answering typing send={send} />)
  return send
}

describe('a question to the person, on the Remote', () => {
  it('shows its choices and answers with the one picked, never with a bare allow', () => {
    const send = asking([question('q1', 'AskUserQuestion', [asked('Which colour?', ['Red', 'Green'])])])
    expect(screen.getByText('Which colour?')).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Allow' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Green' }))
    expect(send).toHaveBeenCalledWith({ t: 'pick', id: 'q1', chosen: [['Green']] })
  })

  it('asks several questions one at a time and sends them together, several picks for a multiple one', () => {
    const send = asking([
      question('q2', 'AskUserQuestion', [asked('Which checks?', ['Lint', 'Tests', 'Build'], true), asked('Ship it?', ['Yes', 'No'])]),
    ])
    expect(screen.getByText('1 of 2')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Lint' }))
    fireEvent.click(screen.getByRole('button', { name: 'Build' }))
    fireEvent.click(screen.getByRole('button', { name: 'Next' }))
    expect(send).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'No' }))
    expect(send).toHaveBeenCalledWith({ t: 'pick', id: 'q2', chosen: [['Lint', 'Build'], ['No']] })
  })

  it('refuses it with Deny', () => {
    const send = asking([question('q3', 'AskUserQuestion', [asked('Which colour?', ['Red'])])])
    fireEvent.click(screen.getByRole('button', { name: 'Deny' }))
    expect(send).toHaveBeenCalledWith({ t: 'answer', id: 'q3', allow: false })
  })

  it('offers Deny alone for one whose choices the machine did not read', () => {
    const send = asking([question('ask_1', 'AskUserQuestion', null, 'terminal')])
    expect(screen.queryByRole('button', { name: 'Allow' })).toBeNull()
    expect(screen.getByText('Answer it on the machine.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Deny' }))
    expect(send).toHaveBeenCalledWith({ t: 'answer', id: 'ask_1', allow: false })
  })

  it('still allows a tool', () => {
    const send = asking([question('q4', 'Bash', null)])
    fireEvent.click(screen.getByRole('button', { name: 'Allow' }))
    expect(send).toHaveBeenCalledWith({ t: 'answer', id: 'q4', allow: true })
  })
})
