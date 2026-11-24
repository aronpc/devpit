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

/** Every value of a kind in `css` written by hand, with its line. */
function raw(css: string, kind: Kind): string[] {
  const [declaration, isRaw] = RAW[kind]
  const bare = css.replace(/\/\*[\s\S]*?\*\//g, (comment) => comment.replace(/[^\n]/g, ' '))
  return bare.split('\n').flatMap((line, at) =>
    [...line.matchAll(declaration)].filter((m) => isRaw(m[1])).map((m) => `line ${at + 1}: ${m[0].trim()}`),
  )
}

/*
 * How many hand-written values each part may still hold. It only goes down:
 * a part that loses some lowers its number here, and a new value fails.
 */
const BUDGET: Record<string, Partial<Record<Kind, number>>> = {
  '01-frame.css': { size: 23, radius: 13, height: 10, motion: 3 },
  '02-sheet.css': { size: 13, radius: 1, motion: 3 },
  '03-sidebar.css': { size: 73, radius: 33, height: 21, motion: 20 },
  '04-tabs.css': { size: 8, radius: 6, height: 5 },
  '05-motion.css': { radius: 1, motion: 6 },
  '06-settings.css': { size: 93, radius: 31, height: 17, motion: 7 },
  '07-board.css': { size: 29, radius: 17, height: 9, motion: 9 },
  '08-capabilities.css': { size: 39, radius: 21, height: 14, motion: 12 },
  '09-tree.css': { size: 21, radius: 9, height: 12, motion: 2 },
  '10-explorer.css': { size: 43, radius: 24, height: 16, motion: 2 },
  '11-onboarding.css': { size: 8, radius: 5, height: 2, motion: 3 },
  '12-editor.css': { size: 8, radius: 7, height: 1 },
  '13-markdown.css': { size: 14, radius: 9, height: 3, motion: 1 },
  '14-chat.css': { height: 22, motion: 14 },
  '15-splits.css': {},
  '16-agent-pane.css': { size: 12, radius: 2, height: 3, motion: 2 },
  '17-worktrees.css': { size: 13, radius: 7, height: 3, motion: 1 },
  '18-card.css': { size: 62, radius: 19, height: 3, motion: 1 },
  '19-bell.css': { size: 6, radius: 2, height: 1, motion: 1 },
  '20-files.css': { size: 8, radius: 3, height: 6 },
  '21-turn.css': { height: 9, motion: 1 },
  '22-plugins.css': { size: 4, radius: 2, height: 2 },
  '23-update.css': { size: 7, radius: 5, height: 2, motion: 1 },
  '24-browser.css': { size: 11, radius: 7, height: 1, motion: 1 },
  '25-git.css': { size: 15, radius: 5, height: 6, motion: 3 },
  '26-rail.css': { size: 25, radius: 15, height: 15, motion: 15 },
  '27-history.css': { size: 9, radius: 4, height: 2, motion: 1 },
  '28-blocks.css': { size: 18, radius: 9, height: 8, motion: 2 },
  '29-fields.css': { radius: 1, motion: 1 },
  '30-devflow.css': { size: 48, radius: 20, height: 8, motion: 4 },
  '31-schedules.css': { size: 3, radius: 2, height: 1 },
  '32-ask.css': { size: 13, radius: 6 },
  '33-glass.css': {},
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

  it('has a budget for every part, and a part for every budget', () => {
    expect(Object.keys(BUDGET).sort()).toEqual(PARTS)
  })

  it.each(PARTS)('%s holds no more hand-written values than its budget', (part) => {
    const css = read(`${STYLES}/${part}`)
    const over = KINDS.flatMap((kind) => {
      const found = raw(css, kind)
      return found.length > (BUDGET[part]?.[kind] ?? 0) ? [`${kind}: ${found.length} > ${BUDGET[part]?.[kind] ?? 0}`, ...found] : []
    })
    expect(over).toEqual([])
  })

  it.each(PARTS)('%s names only tokens that tokens.css defines', (part) => {
    const tokens = read('src/theme/tokens.css')
    const used = new Set([...read(`${STYLES}/${part}`).matchAll(/var\((--(?:fs|fw|lh|track|radius|space|ctl|ctl-r|dur|ease|e\d|focus|icon)[a-z0-9-]*)\)/g)].map((m) => m[1]))
    expect([...used].filter((name) => !tokens.includes(`${name}:`))).toEqual([])
  })
})
