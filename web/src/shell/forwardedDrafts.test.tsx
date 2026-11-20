import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ForwardedRequest, Project, Thread } from '../gen/bindings'
import { ForwardedDrafts } from './ForwardedDrafts'
import { ForwardedMark } from './ForwardedMark'
import { RailOrchestrators } from './RailOrchestrators'

let kept: ForwardedRequest[] = []
let history: Thread[] = []
const destination = { id: 'prj_1', name: 'destination', rootPath: '/tmp/destination', orchestrator: 'prof_1' } as Project
let currentProject: Project | null = null
const dropped = vi.fn(async (_project: string, from: string, id: string) => {
  kept = kept.filter((one) => !(one.from === from && one.id === id))
})
const said = vi.fn()
const sent = vi.fn()
const shown = vi.fn()
const chatList = vi.fn(async () => ({ conversations: history }))
const forwardedList = vi.fn(async () => ({ requests: kept }))
const forwardedSend = vi.fn(async (..._args: unknown[]): Promise<{ conversation: string }> => ({ conversation: 'conv_1' }))
const heard: Array<(id: string) => void> = []

vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => {
    try {
      return { data: await call(), error: null, loading: false }
    } catch (thrown) {
      return { data: null, error: (thrown as { message: string }).message, loading: false }
    }
  },
  commands: {
    forwardedList: () => forwardedList(),
    chatList: () => chatList(),
    forwardedDrop: (...args: [string, string, string]) => dropped(...args),
    forwardedSend: (...args: unknown[]) => forwardedSend(...args),
    chatSay: (...args: unknown[]) => said(...args),
    orchestratorReply: (...args: unknown[]) => sent(...args),
  },
}))
vi.mock('./useShell', () => ({
  useShell: () => ({
    projects: [destination],
    project: currentProject,
    setProject: (id: string) => (currentProject = id === destination.id ? destination : null),
    show: shown,
    open: [],
    reloadProjects: vi.fn(),
  }),
}))
vi.mock('./window', () => ({
  onCarried: (_name: string, then: (id: string) => void) => {
    heard.push(then)
    return () => undefined
  },
}))

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  kept = []
  history = []
  currentProject = null
  heard.length = 0
})

const request = (from: string, id: string, text: string, conversation = 'conv_1'): ForwardedRequest => ({ from, id, text, conversation })

async function openDestination(): Promise<void> {
  const view = render(<RailOrchestrators onMenu={vi.fn()} onRemove={vi.fn()} onEdit={vi.fn()} />)
  fireEvent.click(screen.getByTitle('/tmp/destination'))
  view.rerender(<RailOrchestrators onMenu={vi.fn()} onRemove={vi.fn()} onEdit={vi.fn()} />)
  await waitFor(() => expect(shown).toHaveBeenCalled())
}

describe('requests forwarded to an orchestrator', () => {
  it("a destination with no history opens the request's conversation", async () => {
    kept = [request('orchestrator-a', 'r1', 'please look', 'conv_waiting')]
    await openDestination()
    expect(shown).toHaveBeenLastCalledWith('chat', { id: 'conv_waiting' })
    render(<ForwardedDrafts projectId="prj_1" conversation="conv_waiting" />)
    expect(await screen.findByDisplayValue('please look')).toBeTruthy()
  })

  it('opening sends nothing and starts no turn', async () => {
    kept = [request('orchestrator-a', 'r1', 'please look', 'conv_waiting')]
    await openDestination()
    expect(chatList).toHaveBeenCalledTimes(1)
    expect(forwardedList).toHaveBeenCalled()
    expect(forwardedSend).not.toHaveBeenCalled()
    expect(said).not.toHaveBeenCalled()
    expect(sent).not.toHaveBeenCalled()
    expect(dropped).not.toHaveBeenCalled()
  })

  it('a destination with history opens its own conversation', async () => {
    history = [{ id: 'conv_history', title: 'Earlier work', profile: 'prof_1', model: null, costUsd: 0, lastAt: 10 }]
    kept = [request('orchestrator-a', 'r1', 'please look', 'conv_history')]
    await openDestination()
    expect(shown).toHaveBeenLastCalledWith('chat', { id: 'conv_history' })
    render(<ForwardedDrafts projectId="prj_1" conversation="conv_history" />)
    expect(await screen.findByDisplayValue('please look')).toBeTruthy()
  })

  it('two requests are two cards, each with its origin and editable words', async () => {
    kept = [request('orchestrator-a', '1', 'look at the board'), request('orchestrator-b', '1', 'same id, other sender')]
    render(<ForwardedDrafts projectId="prj_1" conversation="conv_1" />)
    const first = (await screen.findByLabelText('Request from orchestrator-a')) as HTMLTextAreaElement
    expect(first.value).toBe('look at the board')
    expect((screen.getByLabelText('Request from orchestrator-b') as HTMLTextAreaElement).value).toBe('same id, other sender')
    fireEvent.change(first, { target: { value: 'look at the board, then the cards' } })
    expect(first.value).toBe('look at the board, then the cards')
    expect((screen.getByLabelText('Request from orchestrator-b') as HTMLTextAreaElement).value).toBe('same id, other sender')
  })

  it('dropping one leaves the other', async () => {
    kept = [request('orchestrator-a', '1', 'first'), request('orchestrator-a', '2', 'second')]
    render(<ForwardedDrafts projectId="prj_1" conversation="conv_1" />)
    await screen.findAllByRole('button', { name: 'Discard' })
    fireEvent.click(screen.getAllByRole('button', { name: 'Discard' })[0])
    await waitFor(() => expect(dropped).toHaveBeenCalledWith('prj_1', 'orchestrator-a', '1'))
    await waitFor(() => expect(screen.getAllByRole('textbox')).toHaveLength(1))
    expect((screen.getByRole('textbox') as HTMLTextAreaElement).value).toBe('second')
    expect(dropped).toHaveBeenCalledTimes(1)
  })

  it('shows only the requests kept for the conversation that is open', async () => {
    kept = [request('orchestrator-a', '1', 'here'), request('orchestrator-a', '2', 'in another', 'conv_2')]
    render(<ForwardedDrafts projectId="prj_1" conversation="conv_1" />)
    expect(await screen.findByDisplayValue('here')).toBeTruthy()
    expect(screen.queryByDisplayValue('in another')).toBeNull()
  })

  it('send calls forwarded_send with the edited words and nothing else', async () => {
    kept = [request('orchestrator-a', '1', 'first'), request('orchestrator-a', '2', 'second')]
    render(<ForwardedDrafts projectId="prj_1" conversation="conv_1" />)
    await screen.findAllByRole('button', { name: 'Send' })
    fireEvent.change(screen.getAllByRole('textbox')[0], { target: { value: 'first, edited' } })
    fireEvent.click(screen.getAllByRole('button', { name: 'Send' })[0])
    await waitFor(() => expect(forwardedSend).toHaveBeenCalledWith('prj_1', 'orchestrator-a', '1', 'first, edited'))
    expect(forwardedSend).toHaveBeenCalledTimes(1)
    expect(said).not.toHaveBeenCalled()
    expect(sent).not.toHaveBeenCalled()
    expect(dropped).not.toHaveBeenCalled()
  })

  it('a refused send leaves the card, with its words, and says why', async () => {
    kept = [request('orchestrator-a', '1', 'first')]
    forwardedSend.mockRejectedValueOnce({ message: 'another turn runs' })
    render(<ForwardedDrafts projectId="prj_1" conversation="conv_1" />)
    fireEvent.click(await screen.findByRole('button', { name: 'Send' }))
    expect((await screen.findByRole('alert')).textContent).toBe('another turn runs')
    expect((screen.getByRole('textbox') as HTMLTextAreaElement).value).toBe('first')
  })

  it('send and discard wait while a send is pending', async () => {
    kept = [request('orchestrator-a', '1', 'first')]
    let finish: ((value: { conversation: string }) => void) | undefined
    forwardedSend.mockImplementationOnce(
      () => new Promise((resolve) => (finish = resolve)),
    )
    render(<ForwardedDrafts projectId="prj_1" conversation="conv_1" />)

    fireEvent.click(await screen.findByRole('button', { name: 'Send' }))
    await waitFor(() => expect(forwardedSend).toHaveBeenCalledTimes(1))
    expect((screen.getByRole('button', { name: 'Send' }) as HTMLButtonElement).disabled).toBe(true)
    expect((screen.getByRole('button', { name: 'Discard' }) as HTMLButtonElement).disabled).toBe(true)

    finish?.({ conversation: 'conv_1' })
    await waitFor(() => expect((screen.getByRole('button', { name: 'Send' }) as HTMLButtonElement).disabled).toBe(false))
  })

  it('mounting sends nothing', async () => {
    kept = [request('orchestrator-a', '1', 'please do it')]
    render(<ForwardedDrafts projectId="prj_1" conversation="conv_1" />)
    await screen.findByDisplayValue('please do it')
    expect(said).not.toHaveBeenCalled()
    expect(sent).not.toHaveBeenCalled()
    expect(dropped).not.toHaveBeenCalled()
    expect(forwardedSend).not.toHaveBeenCalled()
  })

  it('the sidebar marks the destination, and the mark follows the event', async () => {
    const { container } = render(<ForwardedMark projectId="prj_1" />)
    await waitFor(() => expect(heard.length).toBeGreaterThan(0))
    expect(container.innerHTML).toBe('')
    kept = [request('orchestrator-a', '1', 'one'), request('orchestrator-b', '1', 'two')]
    await act(async () => heard.forEach((then) => then('prj_other')))
    expect(container.innerHTML).toBe('')
    heard.forEach((then) => then('prj_1'))
    expect((await screen.findByRole('status')).textContent).toBe('2')
    expect(screen.getByRole('status').getAttribute('aria-label')).toBe('2 forwarded requests wait')
  })
})
