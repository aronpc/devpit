import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Ask, Frame, Message, TurnEnd } from '../gen/bindings'
import { useChat } from './useChat'
import { PAGE } from './useOlder'

afterEach(() => {
  cleanup()
  joins.length = 0
  woke.handler = null
})

/* What each rejoin was handed, and whether it was left. */
const joins: {
  conversationId: string
  onFrame: (frame: Frame) => void
  leave: ReturnType<typeof vi.fn>
  ends: (was: boolean) => void
}[] = []
const woke: { handler: ((conversationId: string) => void) | null } = { handler: null }
let ending: (end: TurnEnd) => void = () => {}
/* What the chat's own send is told, frame by frame, and what it was asked. */
const sending: { onFrame: (frame: Frame) => void; asked: Ask | null } = { onFrame: () => {}, asked: null }

vi.mock('./chat', async (actual) => ({
  ...(await actual<typeof import('./chat')>()),
  rejoin: (conversationId: string, onFrame: (frame: Frame) => void) => {
    const leave = vi.fn()
    let ends: (was: boolean) => void = () => {}
    const running = new Promise<boolean>((resolve) => (ends = resolve))
    joins.push({ conversationId, onFrame, leave, ends })
    return { running, leave }
  },
  send: (asked: Ask, onFrame: (frame: Frame) => void) => (
    (sending.onFrame = onFrame), (sending.asked = asked), { end: new Promise<TurnEnd>((resolve) => (ending = resolve)) }
  ),
}))
const message = (id: string, text = id): Message =>
  ({ id, turnId: null, role: 'assistant', parts: [{ kind: 'text', text, parent: null }], createdAt: 0, streaming: false }) as Message

/* What the transcript holds, and what it was asked. */
const saved: { messages: Message[]; after: Message[]; found: boolean; hold: Promise<void> | null } = {
  messages: [],
  after: [],
  found: true,
  hold: null,
}
const calls = {
  chatHistory: vi.fn(),
  chatHead: vi.fn(),
  chatAfter: vi.fn(),
  chatRewind: vi.fn(),
  chatEditFrom: vi.fn(),
  chatPaste: vi.fn(),
  chatSteer: vi.fn(),
  chatCancel: vi.fn(),
  chatForceStop: vi.fn(),
}
afterEach(() => {
  saved.messages = []
  saved.after = []
  saved.found = true
  saved.hold = null
  Object.values(calls).forEach((one) => one.mockClear())
})

vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: {
    chatHistory: (projectId: string, id: string) => (
      calls.chatHistory(projectId, id),
      {
        id,
        projectId,
        messages: saved.messages,
        profile: 'claude',
        rewindable: [],
        sessionId: null,
        costUsd: 0,
        cardId: null,
      }
    ),
    chatHead: (projectId: string, id: string) => (
      calls.chatHead(projectId, id),
      { sessionId: 'ses_1', rewindable: ['turn_1'], costUsd: 0.5, context: null }
    ),
    chatAfter: async (projectId: string, id: string, after: string | null, limit: number | null) => {
      calls.chatAfter(projectId, id, after, limit)
      // Read as it stood when asked, however long the answer takes.
      const answer = { messages: saved.after, found: saved.found }
      await saved.hold
      return answer
    },
    chatSteer: (projectId: string, id: string, prompt: string) => (calls.chatSteer(projectId, id, prompt), { ...message('msg_steered'), role: 'user' }),
    chatPaste: (projectId: string, type: string, data: string) => (
      calls.chatPaste(projectId, type, data), { name: 'pasted-1.txt', path: '/h/pasted/pasted-1.txt', kind: 'txt' }
    ),
    chatRewind: (projectId: string, id: string, turnId: string) => (calls.chatRewind(projectId, id, turnId), 'conv_fork'),
    chatEditFrom: (projectId: string, id: string, turnId: string, putBack: boolean) => (calls.chatEditFrom(projectId, id, turnId, putBack), 'conv_edit'),
    chatCancel: (id: string) => (calls.chatCancel(id), null),
    chatForceStop: (id: string) => (calls.chatForceStop(id), true),
    agentProfiles: () => [{ id: 'claude', path: '/bin/claude' }],
    projectSettingsRead: () => ({ own: {}, repo: {}, machine: {}, applied: projectSettings.applied, repoProblem: null }),
    permissionAskFromNow: () => null,
    permissionQuestions: () => ({ questions: [] }),
  },
}))
vi.mock('./window', () => ({
  inTauri: () => true,
  onChatWoke: (then: (conversationId: string) => void) => ((woke.handler = then), () => {}),
  onPermissionAsked: () => () => {},
  onPermissionSettled: () => () => {},
}))
const shell = { project: { id: 'p', rootPath: '/w', orchestrator: null as string | null }, show: vi.fn() }
vi.mock('./shellStore', () => ({ useShellPick: (pick: (from: typeof shell) => unknown) => pick(shell) }))

/* What a new chat's project says it starts with. */
const projectSettings: { applied: { model?: string; permissionMode?: string } } = { applied: {} }
afterEach(() => {
  projectSettings.applied = {}
})

const said = (id: string): Frame => ({
  type: 'opened',
  message: { id, turnId: null, role: 'assistant', parts: [], createdAt: 0, streaming: true } as Message,
})

describe("a chat's joined turns", () => {
  it('write nothing into the next conversation shown in the same chat', async () => {
    const { result, rerender } = renderHook(({ id }) => useChat(id), { initialProps: { id: 'conv_a' } })
    await waitFor(() => expect(joins.map((one) => one.conversationId)).toEqual(['conv_a']))
    rerender({ id: 'conv_b' })
    await waitFor(() => expect(joins.map((one) => one.conversationId)).toEqual(['conv_a', 'conv_b']))

    act(() => {
      joins[0]!.onFrame(said('msg_from_a'))
      joins[1]!.onFrame(said('msg_from_b'))
    })
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toContain('msg_from_b'))
    expect(result.current.messages.get().map((one) => one.id)).not.toContain('msg_from_a')
    expect(joins[0]!.leave).toHaveBeenCalled()
  })

  it('are left when the chat goes', async () => {
    const { unmount } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(joins).toHaveLength(1))
    unmount()
    expect(joins[0]!.leave).toHaveBeenCalled()
  })

  it('take a woken turn after the chat’s own, which would otherwise be heard twice', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    await waitFor(() => expect(joins).toHaveLength(1))
    act(() => result.current.say('hello'))

    act(() => woke.handler?.('conv_a'))
    expect(joins).toHaveLength(1)
    await act(async () =>
      ending({ turnId: 't', costUsd: null, durationMs: null, stopReason: null, isError: false, context: null, unheard: [] }),
    )
    await waitFor(() => expect(joins).toHaveLength(2))
  })
})

describe("a chat's reads of the transcript", () => {
  const end: TurnEnd = { turnId: 't', costUsd: null, durationMs: null, stopReason: null, isError: false, context: null, unheard: [] }

  it('at the end of a turn are of its head, not of the whole conversation', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    expect(calls.chatHistory).toHaveBeenCalledTimes(1)
    act(() => result.current.say('hello'))
    await act(async () => ending(end))
    await waitFor(() => expect(calls.chatHead).toHaveBeenCalledWith('p', 'conv_a'))
    await waitFor(() => expect(result.current.rewindable).toEqual(['turn_1']))
    expect(calls.chatHistory).toHaveBeenCalledTimes(1)
  })

  it('after a rejoined turn append what came after the last message, leaving the rest as drawn', async () => {
    saved.messages = [message('msg_1'), message('msg_2')]
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(joins).toHaveLength(1))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toEqual(['msg_1', 'msg_2']))
    const before = result.current.messages.get()
    act(() => joins[0]!.onFrame(said('msg_3')))
    await waitFor(() => expect(result.current.messages.get()).toHaveLength(3))

    saved.after = [message('msg_3', 'as saved'), message('msg_4')]
    await act(async () => joins[0]!.ends(true))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toEqual(['msg_1', 'msg_2', 'msg_3', 'msg_4']))
    expect(calls.chatAfter).toHaveBeenCalledWith('p', 'conv_a', 'msg_2', PAGE)
    expect(calls.chatHistory).toHaveBeenCalledTimes(1)
    // Not replaced: what was drawn before the join is the same objects.
    expect(result.current.messages.get()[0]).toBe(before[0])
    expect(result.current.messages.get()[1]).toBe(before[1])
    expect(result.current.messages.get()[2]!.parts).toEqual([{ kind: 'text', text: 'as saved', parent: null }])
  })

  it('after a woken turn append from the last message drawn, not from the start', async () => {
    saved.messages = [message('msg_1'), message('msg_2')]
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toEqual(['msg_1', 'msg_2']))
    const before = result.current.messages.get()
    act(() => woke.handler?.('conv_a'))
    await waitFor(() => expect(joins).toHaveLength(2))

    saved.after = [message('msg_3')]
    await act(async () => joins[1]!.ends(true))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toEqual(['msg_1', 'msg_2', 'msg_3']))
    expect(calls.chatAfter).toHaveBeenLastCalledWith('p', 'conv_a', 'msg_2', PAGE)
    expect(result.current.messages.get()[0]).toBe(before[0])
    expect(result.current.messages.get()[1]).toBe(before[1])
  })

  it('after a turn woken behind the chat’s own, read from before that turn: the woken one was saved first', async () => {
    saved.messages = [message('msg_1')]
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    act(() => result.current.say('hello'))
    act(() => woke.handler?.('conv_a'))
    act(() => joins[0]!.onFrame(said('msg_mine')))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toContain('msg_mine'))

    saved.after = [message('msg_woken'), message('msg_mine')]
    await act(async () =>
      ending({ turnId: 't', costUsd: null, durationMs: null, stopReason: null, isError: false, context: null, unheard: [] }),
    )
    await waitFor(() => expect(joins).toHaveLength(2))
    await act(async () => joins[1]!.ends(true))
    expect(calls.chatAfter).toHaveBeenLastCalledWith('p', 'conv_a', 'msg_1', PAGE)
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toContain('msg_woken'))
  })

  it('after a rejoin whose place was never saved, take the last page in place of what was drawn', async () => {
    saved.messages = [message('msg_1'), message('msg_2')]
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toEqual(['msg_1', 'msg_2']))
    act(() => joins[0]!.onFrame(said('never_saved')))
    await waitFor(() => expect(result.current.messages.get()).toHaveLength(3))

    saved.found = false
    saved.after = [message('msg_2'), message('msg_3')]
    await act(async () => joins[0]!.ends(true))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toEqual(['msg_2', 'msg_3']))
    expect(result.current.older).toBe(true)
  })

  it('after a lost place, keep a turn sent while the last page was being read', async () => {
    saved.messages = [message('msg_1'), message('msg_2')]
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    act(() => joins[0]!.onFrame(said('never_saved')))
    await waitFor(() => expect(result.current.messages.get()).toHaveLength(3))

    saved.found = false
    saved.after = [message('msg_1'), message('msg_2')]
    let release: () => void = () => {}
    saved.hold = new Promise<void>((done) => (release = done))
    await act(async () => joins[0]!.ends(true))
    // Sent while the read is still on its way.
    act(() => result.current.say('hello'))
    act(() => sending.onFrame(said('msg_sent')))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toContain('msg_sent'))
    await act(async () => release())
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).not.toContain('never_saved'))
    expect(result.current.messages.get().map((one) => one.id)).toEqual(['msg_1', 'msg_2', 'msg_sent'])
  })

  it('after a lost place, keep a message steered in while the last page was being read', async () => {
    saved.messages = [message('msg_1'), message('msg_2')]
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    saved.found = false
    saved.after = [message('msg_1'), message('msg_2')]
    let release: () => void = () => {}
    saved.hold = new Promise<void>((done) => (release = done))
    await act(async () => joins[0]!.ends(true))
    act(() => result.current.say('hello'))
    act(() => sending.onFrame(said('msg_sent')))
    await act(async () => void (await result.current.steer('more')))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toContain('msg_steered'))
    await act(async () => release())
    await waitFor(() => expect(calls.chatAfter).toHaveBeenCalled())
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toContain('msg_sent'))
    expect(result.current.messages.get().map((one) => one.id)).toContain('msg_steered')
  })

  it('after a lost place, drop a message sent before the read began and never saved', async () => {
    saved.messages = [message('msg_1')]
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    act(() => result.current.say('hello'))
    act(() => sending.onFrame(said('msg_unsaved')))
    await act(async () =>
      ending({ turnId: 't', costUsd: null, durationMs: null, stopReason: null, isError: false, context: null, unheard: [] }),
    )
    await waitFor(() => expect(result.current.sending).toBe(false))

    saved.found = false
    saved.after = [message('msg_1')]
    act(() => woke.handler?.('conv_a'))
    await waitFor(() => expect(joins).toHaveLength(2))
    await act(async () => joins[1]!.ends(true))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toEqual(['msg_1']))
  })

  it('after a lost place, drop what streamed and was never saved, and an older page loaded meanwhile', async () => {
    saved.messages = [message('msg_1'), message('msg_2')]
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toEqual(['msg_1', 'msg_2']))
    saved.found = false
    saved.after = [message('msg_1'), message('msg_2')]
    let release: () => void = () => {}
    saved.hold = new Promise<void>((done) => (release = done))
    // The last batch and the end arrive together, before a render.
    await act(async () => {
      joins[0]!.onFrame(said('never_saved'))
      joins[0]!.ends(true)
    })
    act(() => result.current.took([message('old_0')], true))
    await act(async () => release())
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toEqual(['msg_1', 'msg_2']))
  })

  it('leave rewind working: the head names the turns, and rewinding forks', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    act(() => result.current.say('hello'))
    await act(async () => ending(end))
    await waitFor(() => expect(result.current.rewindable).toEqual(['turn_1']))
    act(() => result.current.rewind('turn_1'))
    await waitFor(() => expect(shell.show).toHaveBeenCalledWith('chat', { id: 'conv_fork' }))
    expect(calls.chatRewind).toHaveBeenCalledWith('p', 'conv_a', 'turn_1')
  })

  it('edit from a turn: a fork from before it opens with its message back in the composer', async () => {
    const asked = { id: 'msg_q', turnId: 'turn_2', role: 'user', parts: [{ kind: 'text', text: '/review\nlook again', parent: null }], createdAt: 1, streaming: false } as Message
    saved.messages = [message('msg_1'), asked, { ...message('msg_a'), turnId: 'turn_2' }]
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.messages.get()).toHaveLength(3))
    act(() => result.current.editFrom('turn_2', true))
    await waitFor(() => expect(shell.show).toHaveBeenCalledWith('chat', { id: 'conv_edit', draft: '/review\nlook again' }))
    expect(calls.chatEditFrom).toHaveBeenCalledWith('p', 'conv_a', 'turn_2', true)
  })
})

describe('a paste of text too long to type', () => {
  it('goes with the message as a file it names, not as typing', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    const megabyte = 'x'.repeat(1024 * 1024)
    act(() => result.current.paste(new Blob([megabyte], { type: 'text/plain' })))
    await waitFor(() => expect(result.current.files).toHaveLength(1))
    expect(calls.chatPaste.mock.calls[0]![1]).toBe('text/plain')
    expect(atob(calls.chatPaste.mock.calls[0]![2] as string)).toHaveLength(megabyte.length)
    expect(result.current.previews).toEqual({})

    act(() => result.current.say('read this'))
    expect(sending.asked?.prompt).toBe('@/h/pasted/pasted-1.txt\nread this')
    expect(result.current.files).toEqual([])
  })
})

describe('a queued message steered into the turn in flight', () => {
  it('goes with its own files, sits before the answer being written, and leaves the composer’s files be', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    act(() => result.current.keep({ name: 'mine.ts', path: 'src/mine.ts', kind: 'ts' }))
    act(() => result.current.say('hello'))
    act(() => sending.onFrame(said('msg_answer')))
    await waitFor(() => expect(result.current.messages.get().map((one) => one.id)).toContain('msg_answer'))

    const picture = { name: 'pasted-2.png', path: '/h/pasted/pasted-2.png', kind: 'png' }
    act(() => result.current.keep({ name: 'next.ts', path: 'src/next.ts', kind: 'ts' }))
    await act(async () => void (await result.current.steer('and this', { files: [picture], skills: [] })))
    expect(calls.chatSteer).toHaveBeenCalledWith('p', 'conv_a', '@/h/pasted/pasted-2.png\nand this')
    const ids = result.current.messages.get().map((one) => one.id)
    expect(ids.indexOf('msg_steered')).toBeLessThan(ids.indexOf('msg_answer'))
    expect(result.current.files.map((one) => one.path)).toEqual(['src/next.ts'])
  })
})

describe('a stop the turn does not heed', () => {
  it('offers to force it 10 s after the stop, forces it, and offers no more once the turn ends', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    vi.useFakeTimers()
    try {
      act(() => result.current.say('hello'))
      act(() => result.current.stop())
      expect(calls.chatCancel).toHaveBeenCalledWith('conv_a')
      act(() => void vi.advanceTimersByTime(9_900))
      expect(result.current.forceable).toBe(false)
      act(() => void vi.advanceTimersByTime(200))
      expect(result.current.forceable).toBe(true)

      act(() => result.current.force())
      expect(calls.chatForceStop).toHaveBeenCalledWith('conv_a')
      await act(async () => ending({ turnId: 't', costUsd: null, durationMs: null, stopReason: 'killed', isError: true, context: null, unheard: [] }))
      expect(result.current.forceable).toBe(false)
    } finally {
      vi.useRealTimers()
    }
  })
})

describe('a stop pressed again', () => {
  it('keeps the first stop’s wait, and a forced stop is offered once', async () => {
    const { result } = renderHook(() => useChat('conv_a'))
    await waitFor(() => expect(result.current.profileId).toBe('claude'))
    vi.useFakeTimers()
    try {
      act(() => result.current.say('hello'))
      act(() => result.current.stop())
      act(() => void vi.advanceTimersByTime(8_000))
      act(() => result.current.stop())
      act(() => void vi.advanceTimersByTime(2_100))
      expect(result.current.forceable).toBe(true)
      act(() => result.current.stop())
      expect(result.current.forceable).toBe(true)

      act(() => result.current.force())
      expect(result.current.forceable).toBe(false)
      await act(async () => ending({ turnId: 't', costUsd: null, durationMs: null, stopReason: 'killed', isError: true, context: null, unheard: [] }))
    } finally {
      vi.useRealTimers()
    }
  })
})

describe('a new chat', () => {
  it("starts with its project's model and permission mode", async () => {
    projectSettings.applied = { model: 'opus', permissionMode: 'plan' }
    const { result } = renderHook(() => useChat('conv_new'))
    await waitFor(() => expect(result.current.model).toBe('opus'))
    await waitFor(() => expect(result.current.permission).toBe('plan'))
  })

  it('starts as before when nothing is set', async () => {
    const { result } = renderHook(() => useChat('conv_plain'))
    await waitFor(() => expect(calls.chatHistory).toHaveBeenCalledWith('p', 'conv_plain'))
    await waitFor(() => expect(result.current.permission).toBe('manual'))
    expect(result.current.model ?? null).toBeNull()
  })

  it('keeps an orchestrator in the mode it listens in, whatever its project says', async () => {
    projectSettings.applied = { model: 'opus', permissionMode: 'manual' }
    shell.project = { ...shell.project, orchestrator: 'claude' }
    try {
      const { result } = renderHook(() => useChat('conv_orch'))
      await waitFor(() => expect(result.current.model).toBe('opus'))
      await waitFor(() => expect(result.current.permission).toBe('acceptEdits'))
    } finally {
      shell.project = { ...shell.project, orchestrator: null }
    }
  })
})
