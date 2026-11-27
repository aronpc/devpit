import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { Toast } from './Toast'

afterEach(cleanup)

describe('a toast', () => {
  it('says what happened and offers the one thing to do', () => {
    const onClick = vi.fn()
    render(<Toast action={{ label: 'Undo', onClick }}>Card archived</Toast>)
    expect(screen.getByRole('status').textContent).toContain('Card archived')
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }))
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('stacks with the others in one place instead of over them', () => {
    render(
      <>
        <Toast>One</Toast>
        <Toast role="alert" tone="danger">Two</Toast>
      </>,
    )
    const one = screen.getByRole('status')
    const two = screen.getByRole('alert')
    expect(one.parentElement).toBe(two.parentElement)
    expect(one.parentElement!.className).toBe('toasts')
    expect(two.getAttribute('data-tone')).toBe('danger')
  })
})
