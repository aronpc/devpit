import { afterEach, describe, expect, it } from 'vitest'

import { setLocale } from '../i18n'
import { reviewerSaid } from './CardDevflow'
import { deleteBody } from './CardHeader'
import { moveTitle } from './LaneMenuView'
import { laneMenu } from './laneMenu'

/* The words of the board, lanes, steps and cards come from the catalog, with
   the plural each language wants: no "(s)" standing in for one. */

afterEach(() => setLocale('en'))

describe('the board counts in the plural of the language', () => {
  it('says one and many apart in English', () => {
    expect(moveTitle(1, 'Doing')).toBe('Move 1 card out of “Doing” first')
    expect(moveTitle(3, 'Doing')).toBe('Move 3 cards out of “Doing” first')
    expect(deleteBody({ comments: 1, pinned: 2, runs: 0, checkout: null })).toBe(
      'Its 1 comment, 2 pinned files and 0 runs go with it. It has no checkout, so nothing on disk changes.',
    )
  })

  it('says them in Portuguese when that is the language', () => {
    setLocale('pt-BR')
    expect(moveTitle(1, 'Fazendo')).toBe('Tire 1 cartão de “Fazendo” primeiro')
    expect(moveTitle(3, 'Fazendo')).toBe('Tire 3 cartões de “Fazendo” primeiro')
    expect(reviewerSaid({ status: 'answered', findings: 2, durationMs: null, costUsd: null } as never)).toBe('2 achados')
    expect(laneMenu({ rename: () => undefined, addCard: () => undefined, shift: () => undefined, remove: () => undefined })[0]).toMatchObject({
      label: 'Renomear',
    })
  })
})
