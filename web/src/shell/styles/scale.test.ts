import { readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

/* Read off disk: `?raw` comes back empty under vitest. */
const read = (path: string): string => readFileSync(resolve(process.cwd(), path), 'utf8')
const STYLES = 'src/shell/styles'
const PARTS = readdirSync(resolve(process.cwd(), STYLES)).filter((name) => name.endsWith('.css')).sort()

type Kind = 'size' | 'radius' | 'height' | 'shadow' | 'motion'

/** A shadow's layers, split at the commas between them and not inside a colour. */
function layers(value: string): string[] {
  const out: string[] = []
  let depth = 0
  let from = 0
  for (let at = 0; at < value.length; at += 1) {
    if (value[at] === '(') depth += 1
    if (value[at] === ')') depth -= 1
    if (value[at] === ',' && depth === 0) {
      out.push(value.slice(from, at))
      from = at + 1
    }
  }
  return [...out, value.slice(from)]
}

/* A layer that is blurred is an elevation, and elevation comes from tokens; a
   ring (no blur) is a border drawn without changing the box, and stays. */
const casts = (layer: string): boolean => {
  const lengths = layer.replace(/var\([^)]*\)|[a-z-]+\([^)]*\)/g, '').match(/-?\d*\.?\d+(px|r?em)?/g) ?? []
  return Number.parseFloat(lengths[2] ?? '0') > 0
}

/*
 * What counts as a value written by hand instead of taken from tokens.css:
 * a font size or radius as a length, a control's height (18 to 48px) as a
 * length, a blurred shadow, a duration or a curve.
 */
const RAW: Record<Kind, [RegExp, (value: string) => boolean]> = {
  size: [/(?<![\w-])(?:font-size|font)\s*:\s*([^;}]+)/g, (v) => /\d(px|r?em|pt)\b/.test(v)],
  radius: [/(?<![\w-])border(?:-[a-z]+)*-radius\s*:\s*([^;}]+)/g, (v) => /\d(px|r?em)\b/.test(v)],
  height: [
    /(?<![\w-])height\s*:\s*([^;}]+)/g,
    (v) => {
      const px = /^\s*(\d+(?:\.\d+)?)px\s*$/.exec(v)
      return px !== null && Number(px[1]) >= 18 && Number(px[1]) <= 48
    },
  ],
  shadow: [/(?<![\w-])box-shadow\s*:\s*([^;}]+)/g, (v) => layers(v).some(casts)],
  motion: [
    /(?<![\w-])(?:transition|animation)(?:-duration|-timing-function|-delay)?\s*:\s*([^;}]+)/g,
    (v) => /(\d|\.)\d*m?s\b|cubic-bezier\(/.test(v),
  ],
}

/** Every value of a kind in `css` written by hand, with its line. A line whose
    comment starts `raw:` and says why it keeps one is the allowlist, let through. */
function raw(css: string, kind: Kind): string[] {
  const [declaration, isRaw] = RAW[kind]
  const kept = new Set(css.split('\n').flatMap((line, at) => (line.includes('/* raw:') ? [at] : [])))
  const bare = css.replace(/\/\*[\s\S]*?\*\//g, (comment) => comment.replace(/[^\n]/g, ' '))
  return bare.split('\n').flatMap((line, at) =>
    kept.has(at)
      ? []
      : [...line.matchAll(declaration)].filter((m) => isRaw(m[1])).map((m) => `line ${at + 1}: ${m[0].trim()}`),
  )
}

const KINDS: Kind[] = ['size', 'radius', 'height', 'shadow', 'motion']

describe("the shell's scales", () => {
  it('catches a value written by hand', () => {
    const literal = [
      '.a { font-size: 12px; border-radius: 6px; font: 600 12.5px var(--mono); border-top-left-radius: 2px; }',
      '.b { height: 28px; box-shadow: 0 1px 2px #000; transition: opacity 120ms ease; animation: x 1.2s infinite; }',
      '.c { transition: color 0.2s cubic-bezier(0.2, 0.7, 0.3, 1); }',
    ].join('\n')
    expect(raw(literal, 'size')).toHaveLength(2)
    expect(raw(literal, 'radius')).toHaveLength(2)
    expect(raw(literal, 'height')).toHaveLength(1)
    expect(raw(literal, 'shadow')).toHaveLength(1)
    expect(raw(literal, 'motion')).toHaveLength(3)
  })

  it('lets a token, a keyword or a size that is not a control through', () => {
    const tokens = [
      '.a { font-size: var(--fs-sm); border-radius: 50%; font: inherit; height: var(--ctl-m); }',
      '.b { height: 1px; min-height: 28px; line-height: 20px; box-shadow: var(--e1), var(--top); box-shadow: none; }',
      '.d { box-shadow: 0 0 0 1px var(--border), inset 0 1px 0 rgb(255 255 255 / 0.1); box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent); }',
      '.c { transition: opacity var(--dur-1) var(--ease-out); /* 120ms */ }',
    ].join('\n')
    for (const kind of KINDS) expect(raw(tokens, kind)).toEqual([])
  })

  it('lets a value through on a line that says why it is kept', () => {
    expect(raw('.a { font-size: 0.87em; } /* raw: sized against the text around it */', 'size')).toEqual([])
    expect(raw('.a { font-size: 0.87em; }', 'size')).toHaveLength(1)
  })

  it.each(PARTS)('%s writes no size, radius, control height, shadow or motion by hand', (part) => {
    const css = read(`${STYLES}/${part}`)
    expect(KINDS.flatMap((kind) => raw(css, kind))).toEqual([])
  })

  it.each(PARTS)('%s names only tokens that tokens.css defines', (part) => {
    const tokens = read('src/theme/tokens.css')
    const used = new Set([...read(`${STYLES}/${part}`).matchAll(/var\((--(?:fs|fw|lh|track|radius|space|ctl|ctl-r|dur|ease|e\d|focus|icon)[a-z0-9-]*)\)/g)].map((m) => m[1]))
    expect([...used].filter((name) => !tokens.includes(`${name}:`))).toEqual([])
  })

  it.each(PARTS)('%s leaves scrollbars to the one mechanism in index.css', (part) => {
    /* A scroller that sets scrollbar-width or scrollbar-color drops the
       ::-webkit-scrollbar rules in Chromium, and two scrollers look different. */
    expect(read(`${STYLES}/${part}`).match(/scrollbar-(width|color)\s*:/g) ?? []).toEqual([])
  })

  it('stops every animation and transition, pseudo-elements included, when less motion is asked for', () => {
    const index = read('src/index.css')
    const reduced = index.slice(index.indexOf('@media (prefers-reduced-motion: reduce)'))
    const block = reduced.slice(0, reduced.indexOf('}\n}') + 3)
    expect(block).toMatch(/\*,\s*\*::before,\s*\*::after\s*\{/)
    expect(block).toMatch(/animation-duration:\s*0\.01ms !important/)
    expect(block).toMatch(/transition-duration:\s*0\.01ms !important/)
  })
})
