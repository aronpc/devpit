import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

import { palette } from '../shell/terminal'

/*
 * The stylesheet is read off disk rather than imported: `?raw` comes back
 * empty under vitest, and a test that asserts about an empty string passes
 * for the wrong reason.
 */
const css = readFileSync(resolve(process.cwd(), 'src/theme/tokens.css'), 'utf8')

/** The declarations inside the first block whose selector matches, by name. */
function valuesIn(selector: string): Map<string, string> {
  const at = css.indexOf(selector)
  expect(at, `${selector} is not in tokens.css`).toBeGreaterThan(-1)
  const open = css.indexOf('{', at)
  let depth = 0
  let close = open
  for (; close < css.length; close += 1) {
    if (css[close] === '{') depth += 1
    if (css[close] === '}') {
      depth -= 1
      if (depth === 0) break
    }
  }
  const body = css.slice(open, close).replace(/\/\*[\s\S]*?\*\//g, '')
  return new Map([...body.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]))
}

const tokensIn = (selector: string): Set<string> => new Set(valuesIn(selector).keys())

/* A token that writes no colour of its own (a size, a face, a curve, or a mix of
   other tokens) is the same in every theme, so it lives on `:root` alone. */
const namesAColour = (value: string): boolean => /#[0-9a-f]{3,8}\b|\b(rgba?|hsla?)\(/i.test(value)

describe('the palette', () => {
  const darkValues = valuesIn(':root {')
  const dark = new Set([...darkValues].filter(([, value]) => namesAColour(value)).map(([name]) => name))

  it('has every dark token in the light theme too', () => {
    const light = tokensIn(":root[data-theme='light']")
    const missing = [...dark].filter((token) => !light.has(token))
    expect(missing, 'a token with no light value paints one theme on the other').toEqual([])
  })

  it('has every dark token in the system theme too', () => {
    const system = tokensIn(":root[data-theme='system']")
    const missing = [...dark].filter((token) => !system.has(token))
    expect(missing).toEqual([])
  })

  it('tells a colour from a step every theme shares', () => {
    expect(namesAColour('#1a1a1a')).toBe(true)
    expect(namesAColour('0 0 0 1px var(--border), 0 1px 2px rgb(0 0 0 / 0.24)')).toBe(true)
    expect(namesAColour('color-mix(in srgb, var(--accent) 14%, transparent)')).toBe(false)
    expect(namesAColour('calc(12px * var(--text-scale, 1))')).toBe(false)
  })

  it('moves every ink and edge toward the text when more contrast is asked for', () => {
    const more = tokensIn(":root[data-contrast='more']")
    const edges = ['--text-2', '--text-3', '--ghost', '--border', '--border-strong', '--border-ctl']
    expect(edges.filter((token) => !more.has(token))).toEqual([])
  })

  it('names more than a handful of tokens', () => {
    /* Guards the reader above: an empty file would satisfy the parity checks. */
    expect(dark.size).toBeGreaterThan(30)
  })

  it("paints the terminal's frame in the ground xterm paints, in each theme", () => {
    expect(valuesIn(':root {').get('--terminal')).toBe(palette(true).background)
    expect(valuesIn(":root[data-theme='light']").get('--terminal')).toBe(palette(false).background)
    expect(valuesIn(":root[data-theme='system']").get('--terminal')).toBe(palette(false).background)
  })
})
