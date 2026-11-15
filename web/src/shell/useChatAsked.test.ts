import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Question } from '../gen/bindings'
import { useChat } from './useChat'

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  waiting.length = 0
  heard.ready = []
  heard.hold = false
  reads.length = 0
  deferRead = false
})

/* What the window hears about questions, so a test can say it. */
const heard: { asked: ((question: Question) => void) | null; settled: ((id: string) => void) | null; ready: (() => void)[]; hold: boolean } = {
  asked: null,
  settled: null,
  ready: [],
  hold: false,
}
/* Reads a test keeps pending until it says so, by session. */
const reads: { session: string; resolve: (questions: Question[]) => void }[] = []
let deferRead = false
/* What the backend says is still waiting, as a chat that opens reads it. */
const waiting: Question[] = []
const picked = vi.fn()
const refused = vi.fn()
const answered = vi.fn()

vi.mock('./chat', async (actual) => ({
  ...(await actual<typeof import('./chat')>()),
  rejoin: () => ({ running: new Promise<boolean>(() => {}), leave: () => {} }),
}))
vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: {
    chatHistory: (projectId: string, id: string) => ({ id, projectId, messages: [], profile: 'claude', rewindable: [], sessionId: id === 'conv_b' ? 'ses_2' : 'ses_1', costUsd: 0, cardId: null }),
    agentProfiles: () => [{ id: 'claude', path: '/bin/claude' }],
    permissionAskFromNow: () => null,
    permissionQuestions: (sessionId: string) =>
      deferRead
        ? new Promise((resolve) => reads.push({ session: sessionId, resolve: (questions) => resolve({ questions }) }))
        : { questions: waiting.filter((one) => one.sessionId === sessionId) },
    permissionPick: (id: string, chosen: string[][]) => (picked(id, chosen), null),
    permissionRefuse: (id: string, reason: string) => (refused(id, reason), null),
    permissionAnswer: (id: string, answer: string) => (answered(id, answer), null),
    chatReceipt: () => null,
  },
}))
vi.mock('./window', () => ({
  inTauri: () => true,
  onChatWoke: () => () => {},
  onPermissionAsked: (then: (question: Question) => void, ready?: () => void) => (
    (heard.asked = then), ready && (heard.hold ? heard.ready.push(ready) : ready()), () => {}
  ),
  onPermissionSettled: (then: (id: string) => void, ready?: () => void) => (
    (heard.settled = then), ready && (heard.hold ? heard.ready.push(ready) : ready()), () => {}
  ),
}))
const shell = { project: { id: 'p', rootPath: '/w', orchestrator: null }, show: vi.fn() }
vi.mock('./shellStore', () => ({ useShellPick: (pick: (from: typeof shell) => unknown) => pick(shell) }))

const bash: Question = { id: 'q1', sessionId: 'ses_1', tool: 'Bash', input: '{"command":"ls"}', cwd: '/w', touch: { kind: 'run', command: 'ls' } }
const colour: Question = {
  id: 'q2',
  sessionId: 'ses_1',
  tool: 'AskUserQuestion',
  input: '{}',
  cwd: '/w',
  touch: { kind: 'ask', questions: [{ header: null, question: 'Which colour?', options: ['Red', 'Green', 'Blue'], multi: false }] },
}

describe("a chat's questions", () => {
  it('go from the chat once the island answered them, a permission and a question alike', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    act(() => {
      heard.asked?.(bash)
      heard.asked?.(colour)
    })
    expect(result.current.asked.map((one) => one.id)).toEqual(['q1', 'q2'])
    act(() => heard.settled?.('q2'))
    expect(result.current.asked.map((one) => one.id)).toEqual(['q1'])
    act(() => heard.settled?.('q1'))
    expect(result.current.asked).toEqual([])
  })

  it('send the choices picked to the question they answer', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    act(() => heard.asked?.(colour))
    act(() => result.current.answer('q2', 'allow', [['Green']]))
    expect(result.current.asked).toEqual([])
    await waitFor(() => expect(picked).toHaveBeenCalledWith('q2', [['Green']]))
  })

  it('send a refusal with what the person said, for the agent to read', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    act(() => heard.asked?.(bash))
    act(() => result.current.answer('q1', 'deny', undefined, '  use rg instead  '))
    await waitFor(() => expect(refused).toHaveBeenCalledWith('q1', 'use rg instead'))
    expect(answered).not.toHaveBeenCalled()
  })

  it('send a refusal with nothing said as a plain refusal', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    act(() => heard.asked?.(bash))
    act(() => result.current.answer('q1', 'deny', undefined, '   '))
    await waitFor(() => expect(answered).toHaveBeenCalledWith('q1', 'deny'))
    expect(refused).not.toHaveBeenCalled()
  })

  it('leave plan mode once its plan is approved, for the mode that asks before changing anything', async () => {
    const plan: Question = { id: 'q3', sessionId: 'ses_1', tool: 'ExitPlanMode', input: '{"plan":"# A"}', cwd: '/w', touch: { kind: 'plan', plan: '# A' } }
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    act(() => result.current.setPermission('plan'))
    act(() => heard.asked?.(plan))
    act(() => result.current.answer('q3', 'deny'))
    expect(result.current.permission).toBe('plan')
    act(() => heard.asked?.(plan))
    act(() => result.current.answer('q3', 'allow'))
    expect(result.current.permission).toBe('manual')
  })

  /* A woken resident can ask with no window listening to the event. */
  it('draw a question asked before the chat was opened, and answer it from there', async () => {
    waiting.push(colour)
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.asked.map((one) => one.id)).toEqual(['q2']))
    act(() => result.current.answer('q2', 'allow', [['Green']]))
    expect(result.current.asked).toEqual([])
    await waitFor(() => expect(picked).toHaveBeenCalledWith('q2', [['Green']]))
  })

  it('draw a question read at open once, not again under the event that was already heard', async () => {
    waiting.push(colour)
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.asked).toHaveLength(1))
    act(() => heard.asked?.(colour))
    expect(result.current.asked.map((one) => one.id)).toEqual(['q2'])
  })

  it('read what is waiting only once both events are heard, so a question asked before that is not lost', async () => {
    heard.hold = true
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    waiting.push(colour)
    await act(async () => heard.ready[0]())
    expect(result.current.asked).toEqual([])
    await act(async () => heard.ready[1]())
    await waitFor(() => expect(result.current.asked.map((one) => one.id)).toEqual(['q2']))
  })

  it('keep a question settled while the read was in flight from coming back with it', async () => {
    deferRead = true
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(reads).toHaveLength(1))
    act(() => heard.settled?.('q2'))
    await act(async () => reads[0].resolve([colour]))
    expect(result.current.asked).toEqual([])
  })

  it('refuse a read that comes back after the chat moved to another conversation', async () => {
    deferRead = true
    const { result, rerender } = renderHook(({ id }) => useChat(id), { initialProps: { id: 'conv_a' } })
    await waitFor(() => expect(reads).toHaveLength(1))
    rerender({ id: 'conv_b' })
    await waitFor(() => expect(reads.length).toBeGreaterThan(1))
    await act(async () => reads[0].resolve([colour]))
    expect(result.current.asked).toEqual([])
  })

  it('drop a question that was asked and settled between the two listeners coming up', async () => {
    heard.hold = true
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    await act(async () => heard.ready[0]())
    act(() => heard.asked?.(colour))
    expect(result.current.asked.map((one) => one.id)).toEqual(['q2'])
    /* Settled with nobody listening for it: the read finds nothing waiting. */
    await act(async () => heard.ready[1]())
    await waitFor(() => expect(result.current.asked).toEqual([]))
  })

  it('keep a question heard before the read when it is still waiting', async () => {
    heard.hold = true
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    await act(async () => heard.ready[0]())
    act(() => heard.asked?.(colour))
    waiting.push(colour)
    await act(async () => heard.ready[1]())
    await waitFor(() => expect(result.current.asked.map((one) => one.id)).toEqual(['q2']))
  })

  it('keep a question of the old conversation off the new one while its session loads', async () => {
    const { result, rerender } = renderHook(({ id }) => useChat(id), { initialProps: { id: 'conv_a' } })
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    rerender({ id: 'conv_b' })
    act(() => heard.asked?.(colour))
    await waitFor(() => expect(result.current.session).toBe('ses_2'))
    expect(result.current.asked).toEqual([])
  })
})

describe('a chat whose project object is renewed', () => {
  it('still shows what is asked and reads what was waiting', async () => {
    const { result, rerender } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    shell.project = { ...shell.project }
    rerender()
    await waitFor(() => expect(result.current.session).toBe('ses_1'))
    act(() => heard.asked?.(bash))
    await waitFor(() => expect(result.current.asked.map((one) => one.id)).toEqual(['q1']))
    waiting.push(colour)
    shell.project = { ...shell.project }
    rerender()
    await waitFor(() => expect(result.current.asked.map((one) => one.id)).toEqual(['q2']))
  })
})
