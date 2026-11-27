import { readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { Icon, ICON_STROKE, iconSize } from './Icon'

afterEach(cleanup)

describe('an icon', () => {
  it('is drawn on the 24 grid in the text colour, with one stroke and round ends', () => {
    const { container } = render(<Icon size="m"><path d="M4 12h16" /></Icon>)
    const svg = container.querySelector('svg')!
    expect(svg.getAttribute('viewBox')).toBe('0 0 24 24')
    expect(svg.getAttribute('stroke')).toBe('currentColor')
    expect(svg.getAttribute('fill')).toBe('none')
    expect(svg.getAttribute('stroke-width')).toBe(String(ICON_STROKE))
    expect(svg.getAttribute('stroke-linecap')).toBe('round')
    expect(svg.getAttribute('stroke-linejoin')).toBe('round')
    expect(svg.getAttribute('class')).toBe('icon icon--m')
    expect(svg.getAttribute('width')).toBe('16')
  })

  it('can be a filled shape, and keeps the class and labels it is given', () => {
    const { container } = render(<Icon size="s" filled className="x" aria-label="pinned"><path d="M0 0h1" /></Icon>)
    const svg = container.querySelector('svg')!
    expect(svg.getAttribute('fill')).toBe('currentColor')
    expect(svg.getAttribute('stroke')).toBe('none')
    expect(svg.getAttribute('class')).toBe('icon icon--s x')
    expect(svg.getAttribute('aria-label')).toBe('pinned')
  })

  it('comes in three sizes, and a drawn size lands on the nearest', () => {
    expect([10, 12, 14, 15, 16, 17, 18, 20].map(iconSize)).toEqual(['s', 's', 's', 'm', 'm', 'm', 'l', 'l'])
  })
})

/* The brand marks and the charts draw their own artwork on their own grid. */
const OWN_ART = new Set(['AgentGlyph.tsx', 'AgentMark.tsx', 'ContextMeter.tsx', 'Icon.tsx', 'Usage.tsx'])

describe('the shell', () => {
  it('draws every icon through Icon', () => {
    const dir = resolve(process.cwd(), 'src/shell')
    const raw = readdirSync(dir)
      .filter((name) => name.endsWith('.tsx') && !name.includes('.test.') && !OWN_ART.has(name))
      .filter((name) => /<svg\b/.test(readFileSync(resolve(dir, name), 'utf8')))
    expect(raw).toEqual([])
  })
})
