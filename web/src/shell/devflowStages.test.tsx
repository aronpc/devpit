import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { DevflowConfig, DevflowStage } from '../gen/bindings'
import { setLocale } from '../i18n'

vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => ({ data: await call(), error: null, loading: false }),
  commands: {},
}))

const { DevflowStages } = await import('./DevflowStages')
const { accuracyLine } = await import('./DevflowAccuracy')
const { decisionsLine } = await import('./devflowDecisions')

const stage = (name: string): DevflowStage => ({
  name,
  uses: `builtin/${name}`,
  own: false,
  publish: null,
  e2eDir: null,
  command: null,
  enabled: true,
  approve: name === 'plan' ? 'person' : null,
  checks: [],
  gate: null,
  rubric: null,
  skill: null,
})

const config = (): DevflowConfig => ({
  enabled: true,
  version: 2,
  teams: [],
  locked: [],
  stagesFrom: 'project',
  stages: [stage('briefing'), stage('plan')],
  reviewers: [],
  blockOn: [],
  consolidate: 'jev',
  decisionsUse: false,
  decisionsMode: 'shadow',
})

afterEach(() => {
  cleanup()
  setLocale('en')
})

describe('the stages, in the person’s language', () => {
  it('say what each stage is in English', () => {
    render(<DevflowStages config={config()} edit={vi.fn()} />)
    expect(screen.getByText('The goal, for whom and what success is, written as an epic card.')).toBeTruthy()
    expect(screen.getByRole('combobox', { name: 'Who approves the plan' })).toBeTruthy()
  })

  it('say it in pt-BR, with the ids the agent reads left alone', () => {
    setLocale('pt-BR')
    const edit = vi.fn()
    render(<DevflowStages config={config()} edit={edit} />)
    expect(screen.getByText('O objetivo, para quem e o que é sucesso, escritos como um cartão de épico.')).toBeTruthy()
    fireEvent.change(screen.getByRole('combobox', { name: 'Quem aprova o estágio plan' }), { target: { value: 'none' } })
    expect(edit).toHaveBeenCalledWith('stages.plan.approve', 'none')
    fireEvent.click(screen.getAllByRole('switch')[0]!)
    expect(edit).toHaveBeenCalledWith('stages.briefing.enabled', false)
  })

  it('say the accuracy and the Decisions line in each language', () => {
    const one = { rubric: 'devflow-done', decisions: 1, labelled: 1, accuracy: 0.5, min: 3, suggestedMin: null, suggestedGrey: null, suggestedAccuracy: null, note: null, held: null }
    expect(accuracyLine(one as never)).toBe('devflow-done: 1 decision, 1 labelled by what you did, agreed 50% at min 3.')
    expect(decisionsLine({ state: 'on', reason: null } as never)).toBe('on — Jev answers')
    setLocale('pt-BR')
    expect(accuracyLine(one as never)).toBe('devflow-done: 1 decisão, 1 rotulada pelo que você fez, concordou 50% com mín. 3.')
    expect(decisionsLine({ state: 'on', reason: null } as never)).toBe('ligadas — o Jev responde')
  })
})
