import { describe, expect, it } from 'vitest'

import {
  DOUBLE_MS,
  closed,
  focused,
  launched,
  many,
  moved,
  replaced,
  nextName,
  opened,
  renamed,
  short,
  sweeping,
  tabTitle,
  titleOf,
  twice,
  type Strip,
  type Tab,
} from './strip'
import { empty } from './tabs'

const tab = (id: string, kind = 'term'): Tab => ({ id, kind: kind as Tab['kind'] })
const strip = (tabs: Tab[], active: string | null = tabs[0]?.id ?? null): Strip =>
  ({ open: tabs, active })

describe('which kinds you can have several of', () => {
  it('lets you open many terminals and chats', () => {
    expect(many('term')).toBe(true)
    expect(many('chat')).toBe(true)
  })

  it('keeps one board', () => {
    expect(many('board')).toBe(false)
  })
})

describe('opening a tab', () => {
  it('appends it and looks at it', () => {
    /* A terminal is named as it opens, so it arrives carrying its number. */
    expect(opened(strip([]), tab('t1'))).toEqual({
      open: [{ ...tab('t1'), title: 'Terminal 1' }],
      active: 't1',
    })
  })

  it('opens a second terminal rather than focusing the first', () => {
    const after = opened(strip([tab('t1')]), tab('t2'))
    expect(after.open.map((t) => t.id)).toEqual(['t1', 't2'])
    expect(after.active).toBe('t2')
  })

  it('focuses the board that is already open instead of adding another', () => {
    const before = strip([tab('b1', 'board'), tab('t1')], 't1')
    const after = opened(before, tab('b2', 'board'))
    expect(after.open).toHaveLength(2)
    expect(after.active).toBe('b1')
  })
})

describe('closing a tab', () => {
  it('lands on the neighbour when it was the active one', () => {
    const after = closed(strip([tab('a'), tab('b'), tab('c')], 'b'), 'b')
    expect(after.active).toBe('c')
  })

  it('leaves you where you were when it was not', () => {
    expect(closed(strip([tab('a'), tab('b')], 'b'), 'a').active).toBe('b')
  })

  it('empties the strip when the last one goes', () => {
    expect(closed(strip([tab('a')]), 'a')).toEqual({ open: [], active: null })
  })

  it('ignores a tab that is not open', () => {
    const before = strip([tab('a')])
    expect(closed(before, 'zz')).toBe(before)
  })
})

describe('dragging a tab', () => {
  it('puts it where it was dropped', () => {
    const after = moved(strip([tab('a'), tab('b'), tab('c')]), 'c', 0)
    expect(after.open.map((t) => t.id)).toEqual(['c', 'a', 'b'])
  })

  it('refuses a landing spot outside the strip', () => {
    const before = strip([tab('a'), tab('b')])
    expect(moved(before, 'a', 5)).toBe(before)
  })
})

describe('what a tab says', () => {
  it('takes the title the terminal reports', () => {
    const after = renamed(strip([tab('a')]), 'a', '~/devpit')
    expect(after.open[0]!.title).toBe('~/devpit')
  })

  it('names the one you are looking at', () => {
    expect(focused(strip([tab('a'), tab('b')], 'b'))?.id).toBe('b')
  })
})

describe('a file is one tab', () => {
  const file = (path: string): Tab => ({ id: `file:${path}`, kind: 'file', path })

  it('opens a second file beside the first', () => {
    const one = opened(empty, file('a.rs'))
    const two = opened(one, file('b.rs'))
    expect(two.open).toHaveLength(2)
    expect(two.active).toBe('file:b.rs')
  })

  it('focuses the tab it already has rather than opening the file twice', () => {
    const one = opened(opened(empty, file('a.rs')), file('b.rs'))
    const again = opened(one, file('a.rs'))
    expect(again.open).toHaveLength(2)
    expect(again.active).toBe('file:a.rs')
  })
})

/* A markdown link opened a file tab with no title, and the strip showed the
   pane's placeholder label `files.rs` over the right file. */
describe('a file tab is named after its file', () => {
  const doc = 'docs/devpit-flow-tools-2026-10.md'
  const file: Tab = { id: `file:${doc}`, kind: 'file', path: doc }

  it('is named after the file when the opener gave it no title', () => {
    expect(opened(empty, file).open[0]!.title).toBe('devpit-flow-tools-2026-10.md')
  })

  it('is named after the file when it was saved before it had a title', () => {
    /* Restored from storage, it never went through `opened`. */
    expect(tabTitle(file)).toBe('devpit-flow-tools-2026-10.md')
  })

  it('keeps a name someone gave it', () => {
    expect(tabTitle({ ...file, title: 'the flow doc' })).toBe('the flow doc')
  })

  it('leaves a tab that is not a file to its own fallback', () => {
    expect(tabTitle({ id: 'c', kind: 'chat' })).toBeUndefined()
  })
})

describe('what a tab is called', () => {
  it('names a conversation after the first thing you said in it', () => {
    expect(titleOf('Fix the login bug')).toBe('Fix the login bug')
  })

  it('takes the first line that has something on it', () => {
    expect(titleOf('\n\n  Boa noite\nand then some more')).toBe('Boa noite')
  })

  it('cuts a long opening line rather than letting it run', () => {
    const said = 'x'.repeat(60)
    expect(titleOf(said)).toBe(`${'x'.repeat(48)}…`)
  })

  it('has nothing to call an empty message', () => {
    /* The caller keeps the fallback; inventing one here would hide it. */
    expect(titleOf('   \n  ')).toBe('')
  })
})

describe('numbering the terminals', () => {
  const term = (title?: string): Tab => ({ id: crypto.randomUUID(), kind: 'term', title })

  it('gives the first one a 1', () => {
    expect(nextName([], 'term', 'Terminal')).toBe('Terminal 1')
  })

  it('counts up past the ones that are open', () => {
    expect(nextName([term('Terminal 1'), term('Terminal 2')], 'term', 'Terminal')).toBe('Terminal 3')
  })

  it('takes back a number the one that closed gave up', () => {
    /* Terminal 4 beside a Terminal 2 reads as two missing terminals. */
    expect(nextName([term('Terminal 2')], 'term', 'Terminal')).toBe('Terminal 1')
  })

  it('ignores a terminal someone renamed', () => {
    expect(nextName([term('deploy')], 'term', 'Terminal')).toBe('Terminal 1')
  })

  it('names a terminal as it opens, and leaves a chat to be named later', () => {
    const one = opened({ open: [], active: null }, { id: 'a', kind: 'term' })
    expect(one.open[0]!.title).toBe('Terminal 1')
    const two = opened(one, { id: 'b', kind: 'term' })
    expect(two.open[1]!.title).toBe('Terminal 2')
    const chat = opened(two, { id: 'c', kind: 'chat' })
    expect(chat.open[2]!.title).toBeUndefined()
  })
})

describe('two clicks close together', () => {
  it('is a rename on the same thing inside the window', () => {
    expect(twice({ id: 't1', at: 0 }, 't1', DOUBLE_MS - 1)).toBe(true)
  })

  it('is two separate clicks once the window has passed', () => {
    expect(twice({ id: 't1', at: 0 }, 't1', DOUBLE_MS + 1)).toBe(false)
  })

  it('is not a double click across two different tabs', () => {
    expect(twice({ id: 't1', at: 0 }, 't2', 1)).toBe(false)
  })

  it('has nothing to compare on the first click', () => {
    expect(twice(null, 't1', 0)).toBe(false)
  })
})

describe('cutting a name to fit', () => {
  it('leaves a name that fits alone', () => {
    expect(short('Terminal 1', 22)).toBe('Terminal 1')
  })

  it('cuts a long one and says it cut it', () => {
    expect(short('x'.repeat(40), 22)).toBe(`${'x'.repeat(22)}…`)
  })

  it('does not leave a space hanging before the ellipsis', () => {
    expect(short('abc def ghi', 4)).toBe('abc…')
  })
})

describe('which tabs a sweep closes', () => {
  const four = (): Strip => ({
    open: [
      { id: 'a', kind: 'file' },
      { id: 'b', kind: 'file' },
      { id: 'c', kind: 'file' },
      { id: 'd', kind: 'file' },
    ] as Tab[],
    active: 'c',
  })

  it('names every tab but the one it was opened on', () => {
    expect(sweeping(four(), 'b', 'others')).toEqual(['a', 'c', 'd'])
  })

  it('names what is to the right, and what is to the left', () => {
    expect(sweeping(four(), 'b', 'right')).toEqual(['c', 'd'])
    expect(sweeping(four(), 'c', 'left')).toEqual(['a', 'b'])
  })

  it('names all of them, including the one it was opened on', () => {
    expect(sweeping(four(), 'a', 'all')).toEqual(['a', 'b', 'c', 'd'])
  })

  /* Ids, not a strip: closing a terminal tab also ends its tmux tree, and
     that happens in `close`. Sabotage: return a rebuilt strip and the tabs
     leave the screen while the shells keep running. */
  it('a tab that is not open names nothing', () => {
    expect(sweeping(four(), 'nope', 'all')).toEqual([])
  })
})

describe('a tab swapped in its own place', () => {
  const strip: Strip = {
    open: [
      { id: 'a', kind: 'term' },
      { id: 'here', kind: 'chat' },
      { id: 'b', kind: 'term' },
    ],
    active: 'here',
  }

  it('takes the place of the one it replaces', () => {
    const next = replaced(strip, 'here', { id: 'earlier', kind: 'chat', title: 'Old talk' })
    expect(next.open.map((tab) => tab.id)).toEqual(['a', 'earlier', 'b'])
    expect(next.active).toBe('earlier')
  })

  it('focuses one already open instead of showing it twice', () => {
    const next = replaced(strip, 'here', { id: 'b', kind: 'term' })
    expect(next.open.map((tab) => tab.id)).toEqual(['a', 'b'])
    expect(next.active).toBe('b')
  })
})

describe('a terminal opened to run something', () => {
  it('stops carrying its sign-in once it was sent, like an agent to launch', () => {
    const strip: Strip = { open: [{ id: 't1', kind: 'term', login: 'codex' }, { id: 't2', kind: 'term', launch: 'claude' }], active: 't1' }
    const sent = launched(launched(strip, 't1'), 't2')
    expect(sent.open.map((tab) => [tab.login, tab.launch])).toEqual([
      [undefined, undefined],
      [undefined, undefined],
    ])
  })
})
