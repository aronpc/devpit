import { readdirSync, readFileSync } from 'node:fs'
import { basename, resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

import { parts, SHEET } from './stylesheet'

const STYLES = resolve(SHEET, '../styles')

describe('the stylesheet in parts', () => {
  /* A part nobody imports is CSS that silently never applies. */
  it('imports every part exactly once, and nothing that is not there', () => {
    const imported = parts().map((part) => basename(part))
    const onDisk = readdirSync(STYLES).filter((name) => name.endsWith('.css'))
    expect([...imported].sort()).toEqual([...onDisk].sort())
    expect(new Set(imported).size).toBe(imported.length)
  })

  /* Order is the cascade, so the numbers in the names have to be the order. */
  it('imports the parts in the order their names give', () => {
    const imported = parts().map((part) => basename(part))
    expect(imported).toEqual([...imported].sort())
  })

  it('keeps rules out of the index, where they would win over every part', () => {
    const rules = readFileSync(SHEET, 'utf8')
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .replace(/@import\s+'[^']+';/g, '')
      .trim()
    expect(rules).toBe('')
  })

  /* Two @keyframes with one name: the later part's wins everywhere, and the
     earlier one's look is lost without a word. */
  it('names every animation once across the parts', () => {
    const names = parts().flatMap((part) => [...readFileSync(part, 'utf8').matchAll(/@keyframes\s+([\w-]+)/g)].map((m) => m[1]))
    expect(names.filter((name, at) => names.indexOf(name) !== at)).toEqual([])
  })

  /* A tool row, an edit and a turn's changes line up as one column of cards. */
  it('gives every card in a turn the one card width', () => {
    const rule = (part: string, selector: string): string => {
      const css = readFileSync(resolve(STYLES, part), 'utf8')
      const at = css.search(new RegExp(`^\\${selector}\\s*\\{`, 'm'))
      return at < 0 ? '' : css.slice(at, css.indexOf('}', at))
    }
    for (const [part, selector] of [['14-chat.css', '.arow'], ['21-turn.css', '.ecard'], ['21-turn.css', '.tchg']]) {
      expect(rule(part, selector), selector).toMatch(/max-width:\s*var\(--chat-card-w\)/)
    }
  })

  /* A card waiting on you says so with a chip in its header; the card itself
     is neutral, so a tinted fill or a coloured edge cannot carry the state. */
  it('keeps the permission, question and draft cards neutral', () => {
    const rules = (part: string): [string, string][] =>
      [...readFileSync(resolve(STYLES, part), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '').matchAll(/([^{}]+)\{([^}]*)\}/g)].map((m) => [m[1].trim(), m[2]])
    const card = /^(\.asking|\.asking__opt(\[aria-pressed='true'\]|:hover)?|\.wprompt|\.wprompt__o(:hover:not\(:disabled\))?|\.rdraft)$/
    const painted = [...rules('14-chat.css'), ...rules('21-turn.css')]
      .filter(([selector]) => selector.split(',').some((one) => card.test(one.trim())))
      .flatMap(([selector, body]) =>
        [...body.matchAll(/(background|border(?:-color)?)\s*:\s*([^;]+)/g)]
          .filter((m) => /--accent|--warning|color-mix/.test(m[2]))
          .map((m) => `${selector} ${m[1]}: ${m[2]}`),
      )
    expect(painted).toEqual([])
  })
})
