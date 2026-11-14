import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProfileModels as Kept } from '../gen/bindings'
import { ProfileModels } from './ProfileModels'
import type { Draft } from './profiles'

let kept: Kept = { modelsOff: [], advisorModel: null }
const write = vi.fn(async (_id: string, next: Kept) => (kept = { ...next, modelsOff: next.modelsOff ?? [], advisorModel: next.advisorModel ?? null }))
vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => ({ data: await call(), error: null, loading: false }),
  commands: {
    profileModelsRead: async () => kept,
    profileModelsWrite: (id: string, next: Kept) => write(id, next),
    agentProfiles: async () => [{ id: 'glm', models: ['default', 'glm-4.6', 'glm-4.5-air'] }],
  },
}))

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  kept = { modelsOff: [], advisorModel: null }
})

const draft: Draft = { id: 'glm', label: 'GLM', base: 'claude', command: '', args: [], env: [], models: [] }

describe("a profile's models in Settings → Providers", () => {
  it('offers every model until one is switched off, and turns it back on', async () => {
    render(<ProfileModels profileId="glm" />)
    const air = await screen.findByRole('checkbox', { name: 'Offer glm-4.5-air' })
    expect((air as HTMLInputElement).checked).toBe(true)
    fireEvent.click(air)
    await waitFor(() => expect(write).toHaveBeenLastCalledWith('glm', { modelsOff: ['glm-4.5-air'], advisorModel: null }))
    await waitFor(() => expect((screen.getByRole('checkbox', { name: 'Offer glm-4.5-air' }) as HTMLInputElement).checked).toBe(false))
    fireEvent.click(screen.getByRole('checkbox', { name: 'Offer glm-4.5-air' }))
    await waitFor(() => expect(write).toHaveBeenLastCalledWith('glm', { modelsOff: [], advisorModel: null }))
  })

  it('never offers to switch off the default model', async () => {
    render(<ProfileModels profileId="glm" />)
    await screen.findByRole('checkbox', { name: 'Offer glm-4.6' })
    expect(screen.queryByRole('checkbox', { name: 'Offer default' })).toBeNull()
  })

  it('keeps an advisor model, and none when emptied', async () => {
    render(<ProfileModels profileId="glm" />)
    const advisor = await screen.findByLabelText('Advisor model')
    await waitFor(() => expect((advisor as HTMLInputElement).disabled).toBe(false))
    fireEvent.blur(advisor, { target: { value: 'opus' } })
    await waitFor(() => expect(write).toHaveBeenLastCalledWith('glm', { modelsOff: [], advisorModel: 'opus' }))
  })

  it("sets the subagent model in a profile of the person's own, and only there", async () => {
    const onDraft = vi.fn()
    render(<ProfileModels profileId="glm" draft={draft} onDraft={onDraft} />)
    fireEvent.change(await screen.findByLabelText('Subagent model'), { target: { value: 'haiku' } })
    expect(onDraft).toHaveBeenLastCalledWith({ ...draft, env: [{ name: 'CLAUDE_CODE_SUBAGENT_MODEL', value: 'haiku' }] })
    cleanup()
    render(<ProfileModels profileId="glm" />)
    await screen.findByLabelText('Advisor model')
    expect(screen.queryByLabelText('Subagent model')).toBeNull()
  })

  it("keeps a default model and effort for any CLI, without Claude Code's advisor", async () => {
    render(<ProfileModels profileId="codex-work" claude={false} />)
    const model = await screen.findByLabelText('Default model')
    await waitFor(() => expect((model as HTMLInputElement).disabled).toBe(false))
    fireEvent.blur(model, { target: { value: 'gpt-5.5' } })
    await waitFor(() => expect(write).toHaveBeenLastCalledWith('codex-work', expect.objectContaining({ model: 'gpt-5.5' })))
    fireEvent.blur(screen.getByLabelText('Default effort'), { target: { value: 'high' } })
    await waitFor(() =>
      expect(write).toHaveBeenLastCalledWith('codex-work', expect.objectContaining({ model: 'gpt-5.5', effort: 'high' })),
    )
    expect(screen.queryByLabelText('Advisor model')).toBeNull()
  })
})
