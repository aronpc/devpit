import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { Empty, TRAY } from './Empty'

afterEach(cleanup)

describe('an empty pane', () => {
  it('leads with its icon, then what is missing, how it fills and the action', () => {
    const { container } = render(
      <Empty icon={TRAY} title="Nothing here" said="Cards land here.">
        <button onClick={vi.fn()}>Add</button>
      </Empty>,
    )
    const box = container.querySelector('.exempty')!
    expect(box.firstElementChild!.getAttribute('class')).toContain('icon')
    expect(screen.getByText('Nothing here').className).toBe('exempty__t')
    expect(screen.getByText('Cards land here.').className).toBe('exempty__d')
    expect(screen.getByRole('button', { name: 'Add' })).toBeTruthy()
  })
})
