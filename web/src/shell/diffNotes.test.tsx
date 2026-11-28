import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import type { CardFindings, Finding } from '../gen/bindings'
import { DiffFiles } from './DiffFiles'
import { falsePositiveOf, newLines, notesOf, tally } from './diffNotes'

const DIFF = [
  'diff --git a/src/main.rs b/src/main.rs',
  '--- a/src/main.rs',
  '+++ b/src/main.rs',
  '@@ -10,3 +10,4 @@ fn main() {',
  ' let a = 1;',
  ' let b = 2;',
  '+let lock = take();',
  ' run(a, b);',
].join('\n')

const finding = (line: number | null, severity: Finding['severity'] = 'blocking'): Finding => ({
  file: 'src/main.rs',
  line,
  severity,
  why: `the lock at ${line} is never released`,
})

const found = (standing: 'current' | 'outdated', findings: Finding[], dismissed: number[] = []): CardFindings['reviews'][number] => ({
  runId: `run_${standing}`,
  found: { findings, standing, atRevision: 'abc1234def', now: 'fff0000', rubric: null },
  dismissed,
})

describe('findings on a card diff', () => {
  it('numbers the new side of a hunk, skipping removed lines', () => {
    const hunk = { header: '@@ -10,3 +10,4 @@', rows: [{ kind: 'same' as const, text: '' }, { kind: 'del' as const, text: '' }, { kind: 'add' as const, text: '' }] }
    expect(newLines(hunk)).toEqual([10, null, 11])
  })

  it('draws a finding on the line it names, and one from another revision as outdated', () => {
    const notes = notesOf({ reviews: [found('current', [finding(12)]), found('outdated', [finding(11, 'worth')])] })
    const { container } = render(<DiffFiles diff={DIFF} notes={notes} />)
    const twelve = container.querySelector('[data-line="12"]')!.parentElement!
    expect(twelve.querySelector('.cdiff__fd')?.textContent).toContain('Blocking')
    const eleven = container.querySelector('[data-line="11"]')!.parentElement!
    const old = eleven.querySelector('.cdiff__fd')!
    expect(old.getAttribute('data-standing')).toBe('outdated')
    expect(old.textContent).toContain('Outdated · made at abc1234def')
  })

  it('offers a Devflow finding as a false positive rather than a dismissal', () => {
    const devflow = { ...found('current', [finding(12)], [0]), runId: 'devflow:card_1/20261011T143005Z' }
    const notes = notesOf({ reviews: [devflow, found('current', [finding(12)])] })
    expect(notes.map(falsePositiveOf)).toEqual([true, false])
    const { container } = render(<DiffFiles diff={DIFF} notes={notes} onDismiss={() => undefined} />)
    const markers = container.querySelectorAll('.cdiff__fd')
    expect(markers[0].textContent).toContain('A false positive, you said')
    expect(markers[0].querySelector('button')?.textContent).toBe('Not a false positive')
    expect(markers[1].querySelector('button')?.textContent).toBe('Dismiss')
  })

  it('keeps a finding on a file the diff does not touch, with its button', () => {
    const devflow = { ...found('current', [{ ...finding(93), file: 'app/Shipping.php' }]), runId: 'devflow:card_1/20261127T195214Z' }
    const { container } = render(<DiffFiles diff={DIFF} notes={notesOf({ reviews: [devflow] })} onDismiss={() => undefined} />)
    const marker = container.querySelector('.cdiff__fd')!
    expect(marker.textContent).toContain('app/Shipping.php')
    expect(marker.querySelector('button')?.textContent).toBe('False positive')
  })

  it('counts only what was not set aside', () => {
    const notes = notesOf({ reviews: [found('current', [finding(12), finding(13, 'noted')], [0])] })
    expect(tally(notes)).toEqual({ blocking: 0, worth: 0, noted: 1 })
  })
})
