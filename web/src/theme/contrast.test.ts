import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

/* Read off disk: `?raw` comes back empty under vitest. */
const css = readFileSync(resolve(process.cwd(), 'src/theme/tokens.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')

function block(selector: string): Map<string, string> {
  const open = css.indexOf('{', css.indexOf(selector))
  const close = css.indexOf('}', open)
  return new Map([...css.slice(open, close).matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]))
}

type Rgb = [number, number, number]

const hex = (value: string): Rgb | null => {
  const m = /^#([0-9a-f]{6})$/i.exec(value)
  return m ? [0, 2, 4].map((at) => parseInt(m[1].slice(at, at + 2), 16)) as Rgb : null
}

/** An `hsl[a](h, s%, l%[, a])` as sRGB and its alpha. */
function hsla(value: string): [Rgb, number] | null {
  const m = /^hsla?\(\s*([\d.]+)\s*,\s*([\d.]+)%\s*,\s*([\d.]+)%\s*(?:,\s*([\d.]+)\s*)?\)$/.exec(value)
  if (!m) return null
  const [h, s, l] = [Number(m[1]), Number(m[2]) / 100, Number(m[3]) / 100]
  const k = (n: number): number => (n + h / 30) % 12
  const a = s * Math.min(l, 1 - l)
  const f = (n: number): number => 255 * (l - a * Math.max(-1, Math.min(k(n) - 3, 9 - k(n), 1)))
  return [[f(0), f(8), f(4)], m[4] === undefined ? 1 : Number(m[4])]
}

/**
 * A token's colour in a theme: a hex, an alias, or a mix of two tokens in sRGB,
 * as the more-contrast block writes them. `ground+layer+layer` paints each
 * layer over the one before: a tint (`color-mix(… N%, transparent)`) or a
 * translucent `hsla`, the way a hover, a selected row or a chip sits on a surface.
 */
function colour(theme: Map<string, string>, name: string): Rgb {
  if (name.includes('+')) {
    const [ground, ...layers] = name.split('+')
    return layers.reduce((base, layer) => {
      const value = theme.get(`--${layer}`) ?? ''
      const over = /^color-mix\(in srgb, var\((--[a-z0-9-]+)\) (\d+)%, transparent\)$/.exec(value)
      const glass = hsla(value)
      if (!over && !glass) throw new Error(`--${layer} is not a layer this test reads`)
      const [ink, share] = over ? [colour(theme, over[1]), Number(over[2]) / 100] : glass!
      return ink.map((v, i) => v * share + base[i] * (1 - share)) as Rgb
    }, colour(theme, ground))
  }
  const value = theme.get(name)
  if (!value) throw new Error(`${name} has no value`)
  const solid = hex(value)
  if (solid) return solid
  const alias = /^var\((--[a-z0-9-]+)\)$/.exec(value)
  if (alias) return colour(theme, alias[1])
  const mix = /^color-mix\(in srgb, var\((--[a-z0-9-]+)\) (\d+)%, var\((--[a-z0-9-]+)\)\)$/.exec(value)
  if (!mix) throw new Error(`${name}: ${value} is not a colour this test reads`)
  const [a, b, share] = [colour(theme, mix[1]), colour(theme, mix[3]), Number(mix[2]) / 100]
  return a.map((v, i) => v * share + b[i] * (1 - share)) as Rgb
}

const luminance = ([r, g, b]: Rgb): number => {
  const lin = (v: number): number => (v / 255 <= 0.03928 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4)
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/** WCAG 2 contrast between two colours. */
function contrast(a: Rgb, b: Rgb): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

const dark = block(':root {')
const light = new Map([...dark, ...block(":root[data-theme='light']")])
const more = block(":root[data-contrast='more']")
const safe = block(":root[data-diff-colours='safe']")
const safeLight = block(":root[data-theme='light'][data-diff-colours='safe']")
const THEMES = {
  dark,
  light,
  'dark, more contrast': new Map([...dark, ...more]),
  'light, more contrast': new Map([...light, ...more]),
  'dark, safe diffs': new Map([...dark, ...safe]),
  'light, safe diffs': new Map([...light, ...safe, ...safeLight]),
}

const SURFACES = ['--canvas', '--sidebar', '--raised', '--raised-2', '--composer', '--inset']
/* What a pair has to reach: text 4.5:1 (AA), a control's edge or an icon 3:1. */
const PAIRS: [string, string[], number][] = [
  ['--text', SURFACES, 4.5],
  ['--text-2', SURFACES, 4.5],
  ['--text-3', SURFACES, 4.5],
  ['--ghost', SURFACES, 3],
  /* The sidebar's counts are --ghost on its ground; a session row's quiet line
     is --text-3, at rest, under the pointer and on the row you are on. */
  ['--ghost', ['--sidebar'], 4.5],
  ['--text-3', ['--sidebar+sidebar-item', '--sidebar+overlay-strong'], 4.5],
  ['--border-ctl', SURFACES, 3],
  ['--accent', ['--canvas', '--sidebar', '--raised', '--inset'], 4.5],
  ['--warning', ['--canvas', '--sidebar', '--raised', '--inset'], 4.5],
  ['--success', ['--canvas', '--sidebar', '--raised', '--inset'], 4.5],
  ['--danger', ['--canvas', '--sidebar', '--raised', '--inset'], 4.5],
  ['--on-accent', ['--accent'], 4.5],
  ['--on-danger', ['--danger'], 4.5],
  ['--text', ['--terminal'], 4.5],
  ['--text-3', ['--terminal'], 4.5],
  ['--accent-ink', ['--canvas+accent-soft', '--raised+accent-soft', '--sidebar+accent-soft', '--sidebar+overlay-strong+accent-soft'], 4.5],
  /* A session's state chip, waiting on someone, in the sidebar. */
  ['--warning-ink', ['--sidebar+warning-soft', '--canvas+warning-soft', '--sidebar+overlay-strong+warning-soft'], 4.5],
  ['--diff-add', ['--canvas', '--raised', '--inset'], 3],
  ['--diff-del', ['--canvas', '--raised', '--inset'], 3],
]

/* Pairs below their bar today, each named so it can only be fixed, never joined. */
const KNOWN_BELOW = new Set<string>([])

function below(): string[] {
  return Object.entries(THEMES).flatMap(([theme, values]) =>
    PAIRS.flatMap(([ink, grounds, need]) =>
      grounds
        .filter((ground) => contrast(colour(values, ink), colour(values, ground)) < need)
        .map((ground) => `${theme}: ${ink} on ${ground}`),
    ),
  )
}

describe('contrast', () => {
  it('measures the way WCAG does', () => {
    expect(contrast([0, 0, 0], [255, 255, 255])).toBeCloseTo(21, 5)
    expect(hsla('hsla(0, 0%, 100%, 0.5)')).toEqual([[255, 255, 255], 0.5])
    expect(hsla('hsl(120, 100%, 25%)')![0].map(Math.round)).toEqual([0, 128, 0])
    expect(contrast([118, 118, 118], [255, 255, 255])).toBeCloseTo(4.54, 2)
  })

  it('keeps every ink above its bar on every surface, in every theme', () => {
    expect(below().filter((pair) => !KNOWN_BELOW.has(pair))).toEqual([])
  })

  it('lists no pair that has since been fixed', () => {
    const now = new Set(below())
    expect([...KNOWN_BELOW].filter((pair) => !now.has(pair))).toEqual([])
  })

  it('parts the sidebar from the canvas in the dark theme', () => {
    expect(contrast(colour(dark, '--sidebar'), colour(dark, '--canvas'))).toBeGreaterThanOrEqual(1.07)
  })
})
