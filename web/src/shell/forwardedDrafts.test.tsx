import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ForwardedRequest } from '../gen/bindings'
import { ForwardedDrafts } from './ForwardedDrafts'
import { ForwardedMark } from './ForwardedMark'

let kept: ForwardedRequest[] = []
const dropped = vi.fn(async (_project: string, from: string, id: string) => {
  kept = kept.filter((one) => !(one.from === from && one.id === id))
})
const said = vi.fn()
const sent = vi.fn()
const heard: Array<(id: string) => void> = []

vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => ({ data: await call(), error: null, loading: false }),
  commands: {
    forwardedList: async () => ({ requests: kept }),
    forwardedDrop: (...args: [string, string, string]) => dropped(...args),
    chatSay: (...args: unknown[]) => said(...args),
    orchestratorReply: (...args: unknown[]) => sent(...args),
  },
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
  heard.length = 0
})

const request = (from: string, id: string, text: string, conversation = 'conv_1'): ForwardedRequest => ({ from, id, text, conversation })

describe('requests forwarded to an orchestrator', () => {
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

  it('mounting sends nothing', async () => {
    kept = [request('orchestrator-a', '1', 'please do it')]
    render(<ForwardedDrafts projectId="prj_1" conversation="conv_1" />)
    await screen.findByDisplayValue('please do it')
    expect(said).not.toHaveBeenCalled()
    expect(sent).not.toHaveBeenCalled()
    expect(dropped).not.toHaveBeenCalled()
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
