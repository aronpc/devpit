import { readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import type { Press } from './keys'
import {
  bindingsOf,
  clashWith,
  defaultKeys,
  firing,
  keysLabel,
  letsGo,
  matches,
  recordKeys,
  resetAllKeys,
  resetKeys,
  SHORTCUT_LIST,
  shortcutNamed,
} from './shortcuts'

afterEach(resetAllKeys)

const SHELL = resolve(process.cwd(), 'src/shell')
const shipped = readdirSync(SHELL).filter((name) => /\.tsx?$/.test(name) && !name.includes('.test.'))
const text = (name: string): string => readFileSync(resolve(SHELL, name), 'utf8')

const press = (key: string, over: Partial<Press> = {}): Press => ({
  key,
  code: '',
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...over,
})

describe('the registry', () => {
  it('gives no key to two shortcuts that can fire at once, on either platform', () => {
    /* `clashWith` is what refuses a recorded key; the defaults pass it too. */
    for (const onMac of [false, true]) {
      const clashes = SHORTCUT_LIST.flatMap((one) =>
        defaultKeys(one.id, onMac).flatMap((spec) => {
          const other = clashWith(one.id, spec, onMac)
          return other ? [`${one.id} ${spec} = ${other.id}`] : []
        }),
      )
      expect(clashes, onMac ? 'on a Mac' : 'off a Mac').toEqual([])
    }
  })

  it('would see a clash where there is one', () => {
    expect(clashWith('palette', 'Mod+T', false)?.id).toBe('terminal')
    // A file's Ctrl+S and the browser's own keys are never in front together.
    expect(clashWith('find', 'Mod+S', false)).toBeNull()
  })

  it('names each shortcut once', () => {
    const ids = SHORTCUT_LIST.map((one) => one.id)
    expect(new Set(ids).size).toBe(ids.length)
  })
})

/*
 * ⌘P was printed on the project button for months and nothing listened for
 * it, and a dozen listeners each read their own keys. A listener now asks the
 * registry; these read the sources to keep it that way.
 */

/** Lines that decide a shortcut by reading the keys themselves. */
function rawKeyReads(source: string): number[] {
  const raw = /\.(?:ctrlKey|metaKey|altKey)\b|\bkey\s*[!=]==?\s*['"](?:F\d{1,2}|Delete|Insert)['"]/
  return source.split('\n').flatMap((line, at) => (raw.test(line) ? [at + 1] : []))
}

/* keys.ts is the matcher. DeskSettings writes the desktop-wide key in the
   global-shortcut plugin's spelling: the desktop's key, not the window's. */
const READS_KEYS = ['keys.ts', 'DeskSettings.tsx']

describe('every shortcut a listener hears', () => {
  it('is read through the registry', () => {
    const found = shipped
      .filter((name) => !READS_KEYS.includes(name))
      .flatMap((name) => rawKeyReads(text(name)).map((line) => `${name}:${line}`))
    expect(found).toEqual([])
  })

  it('would be caught reading its own keys', () => {
    expect(rawKeyReads("if (event.ctrlKey && event.key === 'k') open()")).toEqual([1])
    expect(rawKeyReads("onKeyDown={(event) => event.key === 'F2' && rename()}")).toEqual([1])
    expect(rawKeyReads("if (event.key === 'Escape') close()")).toEqual([])
  })

  /* The other half of the drift: a key in the registry nothing takes. */
  it('has something that takes it', () => {
    const sources = shipped.filter((name) => name !== 'shortcuts.ts').map(text).join('\n')
    const taken = new Set([...sources.matchAll(/\b(?:useShortcut|listen|matches)\(\s*'(\w+)'/g)].map((one) => one[1]))
    expect(SHORTCUT_LIST.map((one) => one.id).filter((id) => !taken.has(id))).toEqual([])
  })
})

/** Every combination a screen prints as a literal: in a title, or quoted.
 *  keys.ts and shortcuts.ts are where the spellings live, so they are not read. */
function announced(): string[] {
  const found: string[] = []
  for (const name of shipped.filter((one) => one !== 'keys.ts' && one !== 'shortcuts.ts')) {
    const source = text(name)
    for (const match of source.matchAll(/title="[^"]*\(((?:[⇧⌥⌃]*⌘|Ctrl\+)[^)]*)\)"/g)) found.push(`${name}: ${match[1]}`)
    for (const match of source.matchAll(/['"`]((?:[⇧⌥⌃]*⌘|Ctrl\+(?:Shift\+)?)[^'"`\s]{1,6})['"`]/g)) found.push(`${name}: ${match[1]}`)
    for (const match of source.matchAll(/>((?:&#8679;|&#8997;)*&#8984;[^<\s]{1,2})</g)) found.push(`${name}: ${match[1]}`)
  }
  return found
}

describe('the keys a screen prints', () => {
  it('come from the registry, so a recorded key is the key printed', () => {
    expect(announced()).toEqual([])
  })

  it('are read from it on the screens that print them', () => {
    const reads = shipped.map(text).join('\n').match(/\bkeys(?:Label)?\(\s*'\w+'/g) ?? []
    expect(reads.length).toBeGreaterThanOrEqual(8)
  })
})

describe('reading a press', () => {
  it('takes ⌘ on a Mac and Ctrl elsewhere, and not the other one', () => {
    expect(matches('palette', press('k', { ctrlKey: true }), false)).toBe(true)
    expect(matches('palette', press('k', { metaKey: true }), false)).toBe(false)
    expect(matches('palette', press('k', { metaKey: true }), true)).toBe(true)
    /* Ctrl+K in a Mac text field deletes to the end of the line. */
    expect(matches('palette', press('k', { ctrlKey: true }), true)).toBe(false)
  })

  it('reads a letter however its case arrives', () => {
    expect(matches('project', press('P', { ctrlKey: true }), false)).toBe(true)
  })

  it('keeps Shift apart on a letter', () => {
    expect(matches('splitRight', press('D', { ctrlKey: true, shiftKey: true }), false)).toBe(true)
    expect(matches('splitRight', press('d', { ctrlKey: true }), false)).toBe(false)
  })

  it('finds the letter under the finger on a layout that types another alphabet', () => {
    expect(matches('palette', press('л', { code: 'KeyK', ctrlKey: true }), false)).toBe(true)
  })

  /* ABNT2: `/` and `?` share a key of their own beside the right Shift, and
     the key a US keyboard calls Slash types `;`. Matched by what it types. */
  it('takes / and ? where a Brazilian keyboard puts them', () => {
    expect(matches('shortcuts', press('/', { code: 'IntlRo', ctrlKey: true }), false)).toBe(true)
    expect(firing(press('?', { code: 'IntlRo', shiftKey: true }), 'elsewhere', false)).toEqual(['shortcuts'])
    expect(matches('shortcuts', press(';', { code: 'Slash', ctrlKey: true }), false)).toBe(false)
    expect(matches('settings', press(',', { code: 'Comma', ctrlKey: true }), false)).toBe(true)
  })

  it('is printed as each platform writes it', () => {
    expect(keysLabel('splitRight', true)).toBe('⇧⌘D')
    expect(keysLabel('splitRight', false)).toBe('Ctrl+Shift+D')
    expect(keysLabel('settings', false)).toBe('Ctrl+,')
  })
})

describe('inside a terminal', () => {
  it('fires only what is allowed there; the rest goes to the program', () => {
    for (const one of SHORTCUT_LIST.filter((shortcut) => !shortcut.inTerminal)) {
      for (const spec of defaultKeys(one.id, false)) {
        const [key, ...held] = spec.split('+').reverse()
        const pressed = press(key === 'Space' ? ' ' : key!, {
          ctrlKey: held.includes('Mod') || held.includes('Ctrl'),
          altKey: held.includes('Alt'),
          shiftKey: held.includes('Shift'),
        })
        expect(firing(pressed, 'terminal', false), `${one.id} ${spec}`).not.toContain(one.id)
      }
    }
    expect(firing(press('k', { ctrlKey: true }), 'elsewhere', false)).toEqual(['palette'])
    expect(firing(press('D', { ctrlKey: true, shiftKey: true }), 'terminal', false)).toEqual(['splitRight'])
  })

  it('lets ⌘ through on a Mac, which no program receives', () => {
    expect(firing(press('k', { metaKey: true }), 'terminal', true)).toEqual(['palette'])
  })

  it('and in a field, leaves a key with no modifier to the typing', () => {
    expect(firing(press('?', { shiftKey: true }), 'field', false)).toEqual([])
    expect(firing(press('?', { shiftKey: true }), 'terminal', false)).toEqual([])
    expect(firing(press('/', { ctrlKey: true }), 'field', false)).toEqual(['shortcuts'])
  })
})

describe('recording a key', () => {
  it('refuses one another shortcut answers to, and names it', () => {
    expect(recordKeys('palette', press('t', { ctrlKey: true }), false)).toEqual({ refused: 'Ctrl+T is already “New terminal”.' })
    expect(bindingsOf('palette', false)).toEqual(['Mod+K'])
  })

  it('keeps a free one, which answers at once and frees the old', () => {
    expect(recordKeys('palette', press('j', { ctrlKey: true }), false)).toEqual({ kept: 'Mod+J' })
    expect(matches('palette', press('j', { ctrlKey: true }), false)).toBe(true)
    expect(matches('palette', press('k', { ctrlKey: true }), false)).toBe(false)
  })

  it('waits while only a modifier is held', () => {
    expect(recordKeys('palette', press('Control', { ctrlKey: true }), false)).toBeNull()
  })

  it('refuses a bare key that walks the window', () => {
    expect(recordKeys('palette', press('Enter'), false)).toEqual({ refused: 'Enter moves around the window. Hold Ctrl or Alt with it.' })
  })

  it('writes a symbol as typed, with the Shift the layout spent on it', () => {
    expect(recordKeys('palette', press('!', { code: 'Digit1', ctrlKey: true, shiftKey: true }), false)).toEqual({ kept: 'Mod+!' })
    expect(keysLabel('palette', false)).toBe('Ctrl+!')
  })

  it('never rebinds a built-in key', () => {
    expect(shortcutNamed('stopTurn').fixed).toBe(true)
    expect(recordKeys('stopTurn', press('x', { ctrlKey: true }), false)).toBeNull()
  })

  it('resets one, unless its default was taken since, and all at once', () => {
    recordKeys('terminal', press('y', { ctrlKey: true }), false)
    recordKeys('palette', press('t', { ctrlKey: true }), false)
    expect(resetKeys('terminal', false)).toBe('Ctrl+T is now “Open the command palette”. Change that one first.')
    expect(resetKeys('palette', false)).toBeNull()
    expect(resetKeys('terminal', false)).toBeNull()
    expect(bindingsOf('terminal', false)).toEqual(['Mod+T'])
    recordKeys('palette', press('j', { ctrlKey: true }), false)
    resetAllKeys()
    expect(bindingsOf('palette', false)).toEqual(['Mod+K'])
  })
})

describe('a held key', () => {
  it('is let go by its own key or by a modifier held with it, on every keyboard', () => {
    expect(letsGo('talk', { key: ' ', code: 'Space' }, false)).toBe(true)
    expect(letsGo('talk', { key: 'Control' }, false)).toBe(true)
    expect(letsGo('talk', { key: 'Shift' }, false)).toBe(true)
    expect(letsGo('talk', { key: 'Meta' }, true)).toBe(true)
    expect(letsGo('talk', { key: 'Alt' }, false)).toBe(false)
    expect(letsGo('talk', { key: 'a', code: 'KeyA' }, false)).toBe(false)
  })

  it('follows the key it was given', () => {
    expect(recordKeys('talk', { key: 'F9' }, false)).toEqual({ kept: 'F9' })
    expect(letsGo('talk', { key: 'F9' }, false)).toBe(true)
    expect(letsGo('talk', { key: ' ', code: 'Space' }, false)).toBe(false)
  })
})
