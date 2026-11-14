import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProjectAgentAnswer } from '../gen/bindings'
import { AskPane } from './AskPane'
import { ProjectPlaces } from './ProjectPlaces'

const called = vi.fn()
const shown = vi.fn()
let answer: ProjectAgentAnswer = {
  answer: 'Tests run with `make test`, defined in Makefile.',
  woke: true,
  cited: ['Makefile'],
}
let here: { id: string; name: string; orchestrator: string | null } = { id: 'prj_api', name: 'api', orchestrator: null }
let labs = true
/* A chat answered by the test itself, when it wants to hold or fail one. */
let chat: ((question: string) => Promise<ProjectAgentAnswer>) | null = null

vi.mock('./live', () => ({
  ask: async (call: () => unknown) => {
    try {
      return { data: await call(), error: null, loading: false }
    } catch (thrown) {
      return { data: null, error: (thrown as Error).message, loading: false }
    }
  },
  commands: new Proxy(
    {},
    {
      get: (_, name) =>
        typeof name !== 'string' || name === 'then'
          ? undefined
          : async (...args: unknown[]) => {
              called(name, ...args)
              if (name === 'maintenanceRead') return { projectAgent: labs }
              if (name === 'projectAgentChat' && chat) return chat(args[1] as string)
              return name === 'projectAgentChat' ? answer : { proposals: [], decisions: [] }
            },
    },
  ),
}))
vi.mock('./useShell', () => ({ useShell: () => ({ project: here, prefs: null }) }))
vi.mock('./shellStore', () => ({ useShellPick: () => shown }))
vi.mock('./MarkdownView', () => ({ Markdown: ({ source }: { source: string }) => <p>{source}</p> }))

afterEach(() => {
  cleanup()
  called.mockReset()
  shown.mockReset()
  here = { id: 'prj_api', name: 'api', orchestrator: null }
  labs = true
  chat = null
})

const asked = async (question: string): Promise<void> => {
  fireEvent.change(screen.getByRole('textbox', { name: 'Ask the project' }), { target: { value: question } })
  fireEvent.keyDown(screen.getByRole('textbox', { name: 'Ask the project' }), { key: 'Enter' })
  await screen.findByText(answer.answer)
}

describe('Ask the project', () => {
  it('is a place of every project, and not of an orchestrator', async () => {
    const open = vi.fn()
    render(<ProjectPlaces open={open} showing={() => false} />)
    await waitFor(() => expect(called).toHaveBeenCalledWith('maintenanceRead'))
    fireEvent.click(screen.getByRole('button', { name: 'Ask the project' }))
    expect(open).toHaveBeenCalledWith('ask')
    cleanup()
    here = { id: 'orch', name: 'Jhol', orchestrator: 'claude' }
    render(<ProjectPlaces open={open} showing={() => false} />)
    expect(screen.queryByRole('button', { name: 'Ask the project' })).toBeNull()
  })

  it('is hidden while Labs has the project agent off', async () => {
    labs = false
    render(<ProjectPlaces open={vi.fn()} showing={() => false} />)
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Ask the project' })).toBeNull())
    expect(screen.getByRole('button', { name: 'Board' })).toBeTruthy()
  })

  /* Each test asks in a project of its own: the pane keeps each project's turns. */
  it('asks the project agent and shows the answer with the files it cites', async () => {
    render(<AskPane />)
    await asked('How are tests run?')
    expect(called).toHaveBeenCalledWith('projectAgentChat', 'prj_api', 'How are tests run?')
    fireEvent.click(screen.getByRole('button', { name: 'Makefile' }))
    expect(shown).toHaveBeenCalledWith('file', { id: 'file:Makefile', path: 'Makefile' })
  })

  it('turns an answer into a fact only as a proposal waiting for approval', async () => {
    here = { id: 'prj_web', name: 'web', orchestrator: null }
    render(<AskPane />)
    await asked('How are tests run?')
    fireEvent.click(screen.getByRole('button', { name: 'Save as fact' }))
    const fact = screen.getByRole('textbox', { name: 'Fact' })
    expect(fact).toHaveProperty('value', 'Tests run with `make test`, defined in Makefile.')
    fireEvent.change(fact, { target: { value: 'Tests run with make test.' } })
    fireEvent.click(screen.getByRole('button', { name: 'Propose' }))
    await screen.findByText(/waits for your approval/)
    expect(called).toHaveBeenCalledWith('projectAgentFact', 'prj_web', 'Tests run with make test.', ['Makefile'])
    expect(called.mock.calls.map(([name]) => name).filter((name) => name !== 'maintenanceRead')).toEqual(['projectAgentChat', 'projectAgentFact'])
  })

  it('offers no fact from an answer that cites no file', async () => {
    answer = { answer: 'I could not tell.', woke: false, cited: [] }
    here = { id: 'prj_cli', name: 'cli', orchestrator: null }
    render(<AskPane />)
    await asked('Who wrote this?')
    expect(screen.queryByRole('button', { name: 'Save as fact' })).toBeNull()
  })

  it('puts an answer that comes after the pane was opened again on its own question', async () => {
    here = { id: 'prj_late', name: 'late', orchestrator: null }
    let answered: (said: ProjectAgentAnswer) => void = () => undefined
    chat = () => new Promise((resolve) => (answered = resolve))
    render(<AskPane />)
    fireEvent.change(screen.getByRole('textbox', { name: 'Ask the project' }), { target: { value: 'Learn the project' } })
    fireEvent.keyDown(screen.getByRole('textbox', { name: 'Ask the project' }), { key: 'Enter' })
    await screen.findByText(/Reading/)
    cleanup()
    render(<AskPane />)
    expect(screen.getByText('Learn the project')).toBeTruthy()
    await act(async () => answered({ answer: 'It is a desktop app.', woke: true, cited: [] }))
    await screen.findByText('It is a desktop app.')
    expect(screen.queryByText(/Reading/)).toBeNull()
  })

  it('says a failed question failed instead of reading forever', async () => {
    here = { id: 'prj_fail', name: 'fail', orchestrator: null }
    chat = async () => {
      throw new Error('the project agent did not answer within 15 minutes: ask again')
    }
    render(<AskPane />)
    fireEvent.change(screen.getByRole('textbox', { name: 'Ask the project' }), { target: { value: 'Anything?' } })
    fireEvent.keyDown(screen.getByRole('textbox', { name: 'Ask the project' }), { key: 'Enter' })
    expect((await screen.findByRole('alert')).textContent).toContain('did not answer')
    expect(screen.queryByText(/Reading/)).toBeNull()
    expect(screen.getByRole('textbox', { name: 'Ask the project' })).toHaveProperty('disabled', false)
  })
})
