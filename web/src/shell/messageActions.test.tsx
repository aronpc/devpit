import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { Message } from '../gen/bindings'
import { putBackFrom } from './chat'
import { chatMessages } from './chatMessages'
import { applied } from './frames'
import { ChatThread } from './ChatThread'
import { Quoting } from './Quoting'
import { Turn } from './Turn'

vi.mock('./TurnApps', () => ({ TurnApps: () => null }))
vi.mock('./TurnDelegations', () => ({ TurnDelegations: () => null }))
const show = vi.fn()
vi.mock('./shellStore', () => ({ useShellPick: (pick: (shell: unknown) => unknown) => pick({ project: { id: 'p' }, show }) }))
vi.mock('./Streamed', () => ({ useStream: () => ({ url: 'media://shot.png', error: null }) }))
const opened = vi.fn()
vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: { pathOpen: (path: string) => (opened(path), { path }), urlOpen: (url: string) => (opened(url), { path: url }) },
}))

const writeText = vi.fn(async () => {})
beforeEach(() => {
  vi.stubGlobal('matchMedia', () => ({ matches: true }))
  vi.stubGlobal('navigator', { clipboard: { writeText } })
})
afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.clearAllMocks()
})

const said = (text: string, turnId = 't2'): Message =>
  ({ id: `u_${turnId}`, turnId, role: 'user', parts: [{ kind: 'text', text, parent: null }], createdAt: 1_700_000_000_000, streaming: false }) as Message
const answer = (over: Partial<Message> = {}): Message =>
  ({
    id: 'a_t2',
    turnId: 't2',
    role: 'assistant',
    parts: [{ kind: 'text', text: 'Done. See src/shell/Turn.tsx:42 for it.', parent: null }],
    createdAt: 1_700_000_000,
    streaming: false,
    ended: { at: 1_700_000_081, stopReason: 'end_turn', isError: false, model: 'claude-opus-4-5-20251101', effort: 'high', costUsd: 0.12, durationMs: 81_000, tokens: 45_000 },
    ...over,
  }) as Message

describe('your own message', () => {
  it('copies as you typed it: no skill line, the files it went with named after', async () => {
    render(<Turn message={said('/devflow-review\n@/tmp/shot.png @src/a.ts\nPlease look again')} />)
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Copy message' })))
    expect(writeText).toHaveBeenCalledWith('Please look again\n\nAttached:\n- /tmp/shot.png\n- src/a.ts')
  })

  it('keeps a command typed on its own, which is the whole message', async () => {
    render(<Turn message={said('/compact')} />)
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Copy message' })))
    expect(writeText).toHaveBeenCalledWith('/compact')
  })

  it('offers to edit from it and to send it again', () => {
    const edit = vi.fn()
    const again = vi.fn()
    render(<Turn message={said('hello')} onEdit={edit} onAgain={again} />)
    fireEvent.click(screen.getByRole('button', { name: 'Edit from here' }))
    fireEvent.click(screen.getByRole('button', { name: 'Send again' }))
    expect(edit).toHaveBeenCalledWith('t2')
    expect(again).toHaveBeenCalledOnce()
  })

  it('opens a picture it went with as large as the window, to open or save', async () => {
    render(<Turn message={said('@/tmp/shot.png\nlook')} />)
    fireEvent.click(screen.getByRole('button', { name: 'Enlarge shot.png' }))
    const zoom = screen.getByRole('dialog', { name: 'shot.png, enlarged' })
    expect(zoom.querySelector('a[download]')?.getAttribute('href')).toBe('media://shot.png')
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Open' })))
    expect(opened).toHaveBeenCalledWith('/tmp/shot.png')
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.queryByRole('dialog')).toBeNull()
  })
})

describe('an answer', () => {
  it('ends on one quiet line: the model, its effort, how long, the context and the cost', () => {
    render(<Turn message={answer()} />)
    const line = document.querySelector('.tmeta')?.textContent ?? ''
    expect(line).toMatch(/· opus-4-5 · high · 1m 21s · 45k tokens · \$0\.12$/)
  })

  it('says its cost is unavailable when the agent counted tokens on a model nobody priced', () => {
    const ended = { at: 1_700_000_009, stopReason: 'completed', isError: false, model: 'gpt-6.1-sol', effort: 'low', costUsd: null, durationMs: 8_664, tokens: 15_353 }
    render(<Turn message={answer({ ended })} />)
    expect(document.querySelector('.tmeta')?.textContent).toMatch(/· gpt-6\.1-sol · low · 9s · 15k tokens · cost n\/a$/)
  })

  it('says the same the moment it ends as it does reopened', () => {
    const live = { ...answer(), streaming: true, ended: undefined } as Message
    const end = { turnId: 't2', costUsd: 0.12, durationMs: 81_000.4, stopReason: 'end_turn', isError: false, context: { used: 45_000, window: 200_000 }, unheard: [], model: 'claude-opus-4-5', effort: 'high' }
    const [ended] = applied([live], { type: 'ended', end })
    render(<Turn message={ended!} />)
    expect(document.querySelector('.tmeta')?.textContent).toMatch(/· opus-4-5 · high · 1m 21s · 45k tokens · \$0\.12$/)
  })

  it('from before the line existed, says what it can and nothing it cannot', () => {
    render(<Turn message={answer({ ended: { at: 1_700_000_030, stopReason: 'end_turn', isError: false } })} />)
    expect(document.querySelector('.tmeta')?.textContent).toMatch(/· 30s$/)
  })

  it('copies, edits from and forks from its hover bar, in place of a lone copy button', () => {
    const edit = vi.fn()
    const fork = vi.fn()
    render(<Turn message={answer()} onEdit={edit} onRewind={fork} />)
    const bar = document.querySelector('.turn__foot .hbar')!
    expect([...bar.querySelectorAll('button')].map((one) => one.getAttribute('aria-label'))).toEqual(['Copy', 'Edit from here', 'Fork from here'])
    fireEvent.click(screen.getByRole('button', { name: 'Edit from here' }))
    fireEvent.click(screen.getByRole('button', { name: 'Fork from here' }))
    expect(edit).toHaveBeenCalledWith('t2')
    expect(fork).toHaveBeenCalledWith('t2')
  })

  it('opens a file:line it names in prose beside the chat, read in its folder', () => {
    const peek = vi.fn()
    render(<Turn message={answer()} folder="/w/card" onOpen={peek} />)
    fireEvent.click(screen.getByRole('button', { name: 'src/shell/Turn.tsx:42' }))
    expect(peek).toHaveBeenCalledWith('/w/card/src/shell/Turn.tsx')
  })

  it('opens a file:line a tool printed, from the row that printed it', () => {
    const peek = vi.fn()
    const grep: Message = answer({
      parts: [
        { kind: 'tool_call', id: 'c1', name: 'Grep', input: '{"pattern":"x"}', state: 'ok', parent: null },
        { kind: 'tool_result', call_id: 'c1', output: 'web/src/a.ts:12: x', is_error: false, parent: null },
      ],
      streaming: true,
      ended: undefined,
    })
    render(<Turn message={grep} folder="/w" onOpen={peek} />)
    fireEvent.click(document.querySelector('.arow__h')!)
    fireEvent.click(screen.getByRole('button', { name: 'web/src/a.ts:12' }))
    expect(peek).toHaveBeenCalledWith('/w/web/src/a.ts')
  })

  it('opens an image a tool returned as large as the window', () => {
    const shot: Message = answer({
      parts: [
        { kind: 'tool_call', id: 'c1', name: 'mcp__shots__take', input: '{}', state: 'ok', parent: null },
        { kind: 'text', text: 'Here it is: ![the page](https://example.invalid/p.png)', parent: null },
      ],
    })
    render(<Turn message={shot} />)
    fireEvent.click(screen.getByRole('button', { name: 'Enlarge the page' }))
    expect(screen.getByRole('dialog', { name: 'the page, enlarged' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Open' })).toBeTruthy()
  })
})

function Thread({ messages, onRetry, onContinue = () => {} }: { messages: readonly Message[]; onRetry: (text: string) => void; onContinue?: () => void }): React.JSX.Element {
  const box = useRef<HTMLDivElement>(null)
  const top = useRef<HTMLDivElement>(null)
  const nothing = (): void => {}
  return (
    <div ref={box}>
      <ChatThread messages={chatMessages(messages)} box={box} top={top} older={false} stalled={false} onMore={nothing} sending={false} error={null} folder={null} rewindable={[]} onRewind={nothing} onEdit={nothing} onOpen={nothing} onContinue={onContinue} onRetry={onRetry} />
    </div>
  )
}

describe('a failed turn', () => {
  const failed = answer({ ended: { at: 2, stopReason: 'error', isError: true, error: 'overloaded' } })

  it('offers Retry, which sends the message it failed on again', () => {
    const retry = vi.fn()
    render(<Thread messages={[said('Fix the build'), failed]} onRetry={retry} />)
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(retry).toHaveBeenCalledWith('Fix the build')
  })

  it('offers Retry beside Continue when no answer was saved, and neither after one that finished', () => {
    const { unmount } = render(<Thread messages={[said('Fix the build')]} onRetry={vi.fn()} />)
    expect(screen.getAllByRole('button', { name: /^(Continue|Retry)$/ }).map((one) => one.textContent)).toEqual(['Continue', 'Retry'])
    unmount()
    render(<Thread messages={[said('Fix the build'), answer()]} onRetry={vi.fn()} />)
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull()
  })
})

describe('an edit from here', () => {
  const changed = (files: string[], before: string | null): Message =>
    answer({ id: `a_${files[0]}`, parts: [{ kind: 'changes', files: files.map((path) => ({ path, added: 1, removed: 0 })), before }] })

  it('offers to put back what that turn and the later ones changed', () => {
    const thread = [said('one', 't1'), changed(['a.ts'], 'tree1'), said('two', 't2'), { ...changed(['a.ts', 'b.ts'], 'tree2'), turnId: 't2' }]
    expect(putBackFrom(thread, 't1')).toBe(2)
    expect(putBackFrom(thread, 't2')).toBe(2)
  })

  it('offers nothing to put back when the files as they were were not kept', () => {
    expect(putBackFrom([said('one', 't1'), changed(['a.ts'], null)], 't1')).toBe(0)
    expect(putBackFrom([said('one', 't1')], 't1')).toBe(0)
  })
})

describe('a selection in an answer', () => {
  it('offers Quote, which hands the words on as a quote', () => {
    const quote = vi.fn()
    function Pane(): React.JSX.Element {
      const box = useRef<HTMLDivElement>(null)
      return (
        <div ref={box}>
          <div className="reply">
            <p>First line</p>
          </div>
          <Quoting box={box} onQuote={quote} />
        </div>
      )
    }
    Range.prototype.getBoundingClientRect = () => ({ top: 100, left: 40, width: 20, height: 10 }) as DOMRect
    const { container } = render(<Pane />)
    const words = container.querySelector('.reply p')!.firstChild!
    const range = document.createRange()
    range.setStart(words, 0)
    range.setEnd(words, 10)
    window.getSelection()!.removeAllRanges()
    window.getSelection()!.addRange(range)
    fireEvent.mouseUp(container.firstChild!)
    fireEvent.click(screen.getByRole('button', { name: 'Quote' }))
    expect(quote).toHaveBeenCalledWith('> First line\n\n')
  })
})
