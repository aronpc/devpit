import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { useIslandOpens } from './useIslandOpens'

const heard = new Map<string, (payload: unknown) => void>()
vi.mock('./window', () => ({
  onCarried: (name: string, then: (payload: unknown) => void) => (heard.set(name, then), () => heard.delete(name)),
}))

/* What Rust holds until the webview asks for it, as this mount would. */
let pending: { projectId: string; paneId: string } | null = null
vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve({ data: call(), error: null, loading: false }),
  commands: { islandPendingPane: () => pending },
}))

describe('the island asking for a session', () => {
  it('opens a chat in its own project, once that project is on screen', () => {
    const setProject = vi.fn()
    const show = vi.fn()
    const { rerender } = renderHook(({ current }) => useIslandOpens(current, setProject, () => {}, show), {
      initialProps: { current: 'prj_a' },
    })
    act(() => heard.get('island:open-chat')?.(['prj_b', 'conv_1']))
    expect(setProject).toHaveBeenCalledWith('prj_b')
    expect(show).not.toHaveBeenCalled()
    rerender({ current: 'prj_b' })
    expect(show).toHaveBeenCalledWith('chat', { id: 'conv_1' })
  })

  it('opens an orchestrator by its project alone, its chat being the project', () => {
    const setProject = vi.fn()
    const show = vi.fn()
    renderHook(() => useIslandOpens('prj_a', setProject, () => {}, show))
    act(() => heard.get('island:open-chat')?.(['prj_orch', null]))
    expect(setProject).toHaveBeenCalledWith('prj_orch')
    expect(show).not.toHaveBeenCalled()
  })

  it('opens a terminal by its pane', () => {
    const openPane = vi.fn()
    renderHook(() => useIslandOpens('prj_a', () => {}, openPane, () => {}))
    act(() => heard.get('island:open-pane')?.(['prj_a', 'leaf_1']))
    expect(openPane).toHaveBeenCalledWith('leaf_1')
  })

  it('opens the pane the launch asked for, before this document was there to hear it', async () => {
    pending = { projectId: 'prj_b', paneId: 'leaf_1' }
    const setProject = vi.fn()
    const openPane = vi.fn()
    await act(async () => {
      renderHook(() => useIslandOpens('prj_a', setProject, openPane, () => {}))
    })
    expect(setProject).toHaveBeenCalledWith('prj_b')
    expect(openPane).toHaveBeenCalledWith('leaf_1')
  })

  it('opens a pending pane in the project on screen without switching to it', async () => {
    pending = { projectId: 'prj_a', paneId: 'leaf_2' }
    const setProject = vi.fn()
    const openPane = vi.fn()
    await act(async () => {
      renderHook(() => useIslandOpens('prj_a', setProject, openPane, () => {}))
    })
    expect(setProject).not.toHaveBeenCalled()
    expect(openPane).toHaveBeenCalledWith('leaf_2')
  })

  it('opens nothing when no pane is pending', async () => {
    pending = null
    const setProject = vi.fn()
    const openPane = vi.fn()
    await act(async () => {
      renderHook(() => useIslandOpens('prj_a', setProject, openPane, () => {}))
    })
    expect(setProject).not.toHaveBeenCalled()
    expect(openPane).not.toHaveBeenCalled()
  })
})
