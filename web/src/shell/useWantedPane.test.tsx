import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import type { Tab } from './strip'
import { useWantedPane } from './useWantedPane'

const card: Tab = { id: 'tab_card', kind: 'term', panes: ['leaf_card'] }
const session: Tab = { id: 'tab_session', kind: 'term', panes: ['leaf_session'] }

describe('a pane asked for from another view', () => {
  it('focuses the tab that holds it, and opens none', () => {
    const focus = vi.fn()
    const { result } = renderHook(({ open }) => useWantedPane(open, focus), { initialProps: { open: [card, session] } })
    act(() => result.current('leaf_session'))
    expect(focus).toHaveBeenCalledTimes(1)
    expect(focus).toHaveBeenCalledWith('tab_session')
  })

  it('waits for the tab holding it to arrive, and focuses that one', () => {
    const focus = vi.fn()
    const { result, rerender } = renderHook(({ open }) => useWantedPane(open, focus), { initialProps: { open: [card] } })
    act(() => result.current('leaf_session'))
    expect(focus).not.toHaveBeenCalled()
    rerender({ open: [card, session] })
    expect(focus).toHaveBeenCalledTimes(1)
    expect(focus).toHaveBeenCalledWith('tab_session')
  })
})
