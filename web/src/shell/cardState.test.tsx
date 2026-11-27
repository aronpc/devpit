import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { CardState } from './CardState'

afterEach(cleanup)

describe("a card's state", () => {
  it('says waiting with an icon and a word, not a colour alone', () => {
    const { container } = render(<CardState state="waiting" />)
    const chip = container.querySelector('.cstate')!
    expect(chip.getAttribute('data-state')).toBe('waiting')
    expect(chip.querySelector('svg')).not.toBeNull()
    expect(screen.getByText('Waiting on you')).toBeTruthy()
  })

  it('names a draft plainly', () => {
    render(<CardState state="draft" />)
    expect(screen.getByText('Draft')).toBeTruthy()
  })
})
