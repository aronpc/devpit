import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Attachment, Conversation, Frame, Message, Profile } from '../gen/bindings'
import { setLocale } from '../i18n'
import {
  applied,
  appliedAll,
  ASKS,
  batched,
  steered,
  choices,
  clock,
  fixedTo,
  effortName,
  modes,
  modeName,
  money,
  ready,
  spent,
  withFiles,
  unanswered,
  settled,
} from './chat'

const message = (id: string, streaming = true): Message =>
  ({ id, turnId: null, role: 'assistant', parts: [], createdAt: 0, streaming })

const opened = (id: string): Frame => ({ type: 'opened', message: message(id) })
const text = (id: string, t: string): Frame =>
  ({ type: 'part', message_id: id, part: { kind: 'text', text: t, parent: null } } as Frame)

describe('what arrives on the stream', () => {
  it('adds the message that opened', () => {
    expect(applied([], opened('m1')).map((m) => m.id)).toEqual(['m1'])
  })

  it('grows the open message rather than adding another', () => {
    const after = applied(applied([], opened('m1')), text('m1', 'hi'))
    expect(after).toHaveLength(1)
    expect(after[0]!.parts).toEqual([{ kind: 'text', text: 'hi', parent: null }])
  })

  it('joins consecutive text into one paragraph', () => {
    /* One part per chunk would draw a new block for every few characters. */
    let msgs = applied([], opened('m1'))
    msgs = applied(msgs, text('m1', 'he'))
    msgs = applied(msgs, text('m1', 'llo'))
    expect(msgs[0]!.parts).toEqual([{ kind: 'text', text: 'hello', parent: null }])
  })

  it('keeps a tool call separate from the text around it', () => {
    let msgs = applied([], opened('m1'))
    msgs = applied(msgs, text('m1', 'running'))
    msgs = applied(msgs, {
      type: 'part',
      message_id: 'm1',
      part: { kind: 'tool_call', id: 'c1', name: 'Bash', input: '{}', state: 'running', parent: null },
    } as Frame)
    expect(msgs[0]!.parts).toHaveLength(2)
  })

  it('moves a call on without touching the rest', () => {
    let msgs = applied([], opened('m1'))
    msgs = applied(msgs, {
      type: 'part',
      message_id: 'm1',
      part: { kind: 'tool_call', id: 'c1', name: 'Bash', input: '{}', state: 'running', parent: null },
    } as Frame)
    msgs = applied(msgs, { type: 'call_state', call_id: 'c1', state: 'ok' } as Frame)
    expect(msgs[0]!.parts[0]).toMatchObject({ kind: 'tool_call', state: 'ok' })
  })

  it('stops the spinner when the turn ends', () => {
    /* The end is an event; the absence of frames is not an ending. */
    const msgs = applied(applied([], opened('m1')), {
      type: 'ended',
      end: { turnId: 't', costUsd: null, durationMs: null, stopReason: null, isError: false },
    } as Frame)
    expect(msgs[0]!.streaming).toBe(false)
  })

  it('draws a message steered into the turn before the answer still being written, which keeps growing', () => {
    /* Heard by a window or a phone that did not steer it: the turn's own relay sends it. */
    const said: Message = { ...message('u2', false), role: 'user', turnId: null }
    const after = appliedAll([message('u1', false), message('a1')], [text('a1', 'Look'), { type: 'opened', message: said }, text('a1', 'ing.')])
    expect(after.map((one) => one.id)).toEqual(['u1', 'u2', 'a1'])
    expect(after[2]!.parts).toEqual([{ kind: 'text', text: 'Looking.', parent: null }])
  })

  it('ignores a frame for a message it does not have', () => {
    expect(applied([], text('nope', 'x'))).toEqual([])
  })
})

describe('a thought on the stream', () => {
  const thought = (ms: number | null, parent: string | null = null): Frame => ({
    type: 'part',
    message_id: 'a1',
    part: { kind: 'thinking', text: '', parent, duration_ms: ms },
  })

  it('is closed in place with how long it took, even with a subagent between', () => {
    const between: Frame = { type: 'part', message_id: 'a1', part: { kind: 'text', text: 'inner', parent: 'toolu_1' } }
    const after = appliedAll([message('a1')], [thought(null), between, thought(4200)])
    expect(after[0]!.parts).toEqual([
      { kind: 'thinking', text: '', parent: null, duration_ms: 4200 },
      { kind: 'text', text: 'inner', parent: 'toolu_1' },
    ])
  })
})

describe('which profile the composer may use', () => {
  const profile = (id: string, path: string | null): Profile => ({
    id,
    label: id,
    command: id,
    driver: 'claude',
    path,
    reach: path === null ? 'missing' : 'runnable',
    models: ['default'],
  })

  const shellOnly = (id: string): Profile => ({ ...profile(id, null), reach: 'shell_only' })

  it('offers every runnable profile before the first turn', () => {
    const all = [profile('claude', '/usr/bin/claude'), profile('claude2', '/usr/bin/claude2')]
    expect(choices(all, null).map((one) => one.id)).toEqual(['claude', 'claude2'])
  })

  it('leaves out a profile nothing can start', () => {
    const all = [profile('claude', '/usr/bin/claude'), profile('codex', null)]
    expect(choices(all, null).map((one) => one.id)).toEqual(['claude'])
  })

  it('leaves out a shell function, which the terminal can start and this cannot', () => {
    const all = [profile('claude', '/usr/bin/claude'), shellOnly('glm')]
    expect(choices(all, null).map((one) => one.id)).toEqual(['claude'])
  })

  it('offers only the one the conversation belongs to', () => {
    const all = [profile('claude', '/usr/bin/claude'), profile('claude2', '/usr/bin/claude2')]
    expect(choices(all, 'claude2').map((one) => one.id)).toEqual(['claude2'])
  })

  it('reads a conversation with no turns as belonging to nobody', () => {
    expect(fixedTo({ profile: '' } as Conversation)).toBeNull()
  })

  it('reads a conversation that has spoken as fixed', () => {
    expect(fixedTo({ profile: 'claude2' } as Conversation)).toBe('claude2')
  })
})

describe('whether the send button does anything', () => {
  it('does nothing while a turn is running', () => {
    expect(ready('hello', true, 'claude')).toBe(false)
  })

  it('does nothing with nothing to say', () => {
    expect(ready('   ', false, 'claude')).toBe(false)
  })

  it('does nothing with no profile picked', () => {
    expect(ready('hello', false, null)).toBe(false)
  })

  it('sends when there is a profile and something to say', () => {
    expect(ready('hello', false, 'claude')).toBe(true)
  })
})

describe('what a conversation cost', () => {
  it('says nothing before anything was spent', () => {
    expect(money(0)).toBeNull()
  })

  it('keeps four places while the number is smaller than a cent', () => {
    expect(money(0.0042)).toBe('$0.0042')
  })

  it('rounds to cents once there are cents to round', () => {
    expect(money(1.239)).toBe('$1.24')
  })

  it('is unavailable, not nothing, for an agent that reports no cost', () => {
    expect(spent(0, false)).toBe('n/a')
    expect(spent(0, true)).toBeNull()
    expect(spent(0.5, false)).toBe('$0.50')
  })
})

describe('what the agent is actually sent', () => {
  const file = (path: string): Attachment => ({ name: path, path, kind: 'rs' })

  it('sends the prompt untouched when nothing is attached', () => {
    expect(withFiles('why is this slow?', [])).toBe('why is this slow?')
  })

  it('names each attached file as a path the agent can open', () => {
    expect(withFiles('why?', [file('src/a.rs'), file('src/b.rs')])).toBe(
      '@src/a.rs @src/b.rs\nwhy?',
    )
  })
})

describe('what the agent may do without asking', () => {
  it('offers the mode that stops to ask, now that there is somewhere to answer', () => {
    expect(modes().map((mode) => mode.id)).toContain('manual')
  })

  it('knows which mode stops to ask', () => {
    expect(ASKS('manual')).toBe(true)
    expect(ASKS('acceptEdits')).toBe(false)
    expect(ASKS('bypassPermissions')).toBe(false)
  })
})

describe('what a permission mode says it does', () => {
  /* "Ask" names nothing on its own; the row exists so you know what you are
     agreeing to. */
  it('every mode explains itself', () => {
    for (const mode of modes()) {
      expect(mode.what.length).toBeGreaterThan(10)
    }
  })

  it('names a mode by its label rather than the CLI token', () => {
    expect(modeName('bypassPermissions')).toBe('Full access')
    expect(modeName('manual')).toBe('Supervised')
  })
})

describe('how hard the agent is asked to think', () => {
  it('says Extra high rather than xhigh, which is an argument', () => {
    expect(effortName('xhigh')).toBe('Extra high')
  })

  it('leaves a level it does not know alone', () => {
    expect(effortName('ultracode')).toBe('ultracode')
  })
})

describe('who is speaking', () => {
  it("keeps a subagent's words apart from the agent's", () => {
    // Merged, the subagent's report would read as the agent's own answer.
    const opened = applied([], { type: 'opened', message: { id: 'm', turnId: null, role: 'assistant', parts: [], createdAt: 0, streaming: true } } as Frame)
    const mine = applied(opened, { type: 'part', message_id: 'm', part: { kind: 'text', text: 'I will ask a helper. ', parent: null } } as Frame)
    const theirs = applied(mine, { type: 'part', message_id: 'm', part: { kind: 'text', text: 'notes.txt has 3 lines', parent: 'toolu_agent' } } as Frame)
    expect(theirs[0]!.parts).toHaveLength(2)
    expect(theirs[0]!.parts[1]).toMatchObject({ parent: 'toolu_agent' })
  })
})


describe('a turn the app closed on', () => {
  const said = (role: 'user' | 'assistant'): Message =>
    ({ id: role, turnId: 't', role, parts: [], createdAt: 0, streaming: false }) as Message

  it('is the person speaking last with nothing after it', () => {
    expect(unanswered([said('user')], false)).toBe(true)
    expect(unanswered([said('user'), said('assistant')], false)).toBe(false)
  })

  it('is not a turn still running, nor an empty conversation', () => {
    expect(unanswered([said('user')], true)).toBe(false)
    expect(unanswered([], false)).toBe(false)
  })
})

describe('a rejoined turn', () => {
  it('does not show again a message the transcript already holds', () => {
    const held = applied([], opened('msg_1'))
    expect(applied(held, opened('msg_1'))).toHaveLength(1)
    expect(applied(held, opened('msg_2'))).toHaveLength(2)
  })
})

describe('frames between paints', () => {
  it('are applied even when no animation frame ever comes', () => {
    vi.useFakeTimers()
    vi.stubGlobal('document', { hidden: false })
    vi.stubGlobal('requestAnimationFrame', () => 1)
    vi.stubGlobal('cancelAnimationFrame', () => undefined)
    const got: Frame[][] = []
    const { push } = batched((frames) => got.push([...frames]))
    push(text('a', 'one'))
    push(text('a', 'two'))
    expect(got).toEqual([])
    vi.advanceTimersByTime(100)
    expect(got).toEqual([[text('a', 'one'), text('a', 'two')]])
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })

  it('are applied once the frame they waited for is painted, not inside it', () => {
    vi.useFakeTimers()
    vi.stubGlobal('document', { hidden: false })
    const frames: FrameRequestCallback[] = []
    vi.stubGlobal('requestAnimationFrame', (paint: FrameRequestCallback) => frames.push(paint))
    vi.stubGlobal('cancelAnimationFrame', () => undefined)
    const got: Frame[][] = []
    const { push } = batched((frames) => got.push([...frames]))
    push(text('a', 'one'))

    frames.shift()!(0)
    const inside = got.length
    vi.advanceTimersByTime(0)

    expect(inside).toBe(0)
    expect(got).toEqual([[text('a', 'one')]])
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })
})

describe('a message sent into the turn in flight', () => {
  it('sits before the answer still being written', () => {
    const asked = { ...message('u1', false), role: 'user' as const }
    const said = { ...message('u2', false), role: 'user' as const }
    expect(steered([asked, message('a1')], said).map((one) => one.id)).toEqual(['u1', 'u2', 'a1'])
    expect(steered([message('a0', false)], said).map((one) => one.id)).toEqual(['a0', 'u2'])
  })

  it('is drawn once when the turn sent it back before the steer answered', () => {
    const said: Message = { ...message('u2', false), role: 'user', turnId: null }
    const heard = appliedAll([message('a1')], [{ type: 'opened', message: said }])
    expect(steered(heard, said).map((one) => one.id)).toEqual(['u2', 'a1'])
  })
})

describe('a turn that is over', () => {
  it('leaves no answer still streaming, whatever ended it', () => {
    const was = [message('asked', false), message('answer', true)]
    const after = settled(was)
    expect(after.map((m) => m.streaming)).toEqual([false, false])
    expect(settled(after)).toBe(after)
  })
})

/* The time under a turn in the language the person chose, not the machine's:
   a 24-hour clock in pt-BR even on a runner under LC_ALL=en_US. */
describe("a turn's clock in the chosen language", () => {
  const at = new Date(2026, 0, 20, 15, 5).getTime()
  afterEach(() => setLocale('en'))

  it('reads in pt-BR when pt-BR is chosen', () => {
    setLocale('pt-BR')
    expect(clock(at)).toBe('15:05')
  })

  it('reads in English when English is chosen', () => {
    expect(clock(at)).toMatch(/^03:05\sPM$/)
  })
})
