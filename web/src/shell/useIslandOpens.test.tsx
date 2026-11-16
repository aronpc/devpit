import { StrictMode } from 'react'
import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { useIslandOpens } from './useIslandOpens'

const heard = new Map<string, (payload: unknown) => void>()
/* The listener's registration, already in place unless a test holds it back —
   the state one in flight is. */
let inFlight: Promise<void> | null = null
vi.mock('./window', () => ({
  onCarried: (name: string, then: (payload: unknown) => void) => (heard.set(name, then), () => heard.delete(name)),
  whenHeard: () => inFlight ?? Promise.resolve(),
}))

beforeEach(() => {
  inFlight = null
})

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

  it('asks for the pending pane only once its listener is in place', async () => {
    let landed: () => void = () => {}
    inFlight = new Promise<void>((resolve) => {
      landed = resolve
    })
    pending = { projectId: 'prj_b', paneId: 'leaf_1' }
    const setProject = vi.fn()
    const openPane = vi.fn()
    await act(async () => {
      renderHook(() => useIslandOpens('prj_a', setProject, openPane, () => {}))
    })
    expect(openPane).not.toHaveBeenCalled()
    await act(async () => {
      landed()
    })
    expect(setProject).toHaveBeenCalledWith('prj_b')
    expect(openPane).toHaveBeenCalledWith('leaf_1')
  })

  it('asks for the pending pane once under StrictMode, after the listener is up', async () => {
    let landed: () => void = () => {}
    inFlight = new Promise<void>((resolve) => {
      landed = resolve
    })
    pending = { projectId: 'prj_b', paneId: 'leaf_1' }
    const setProject = vi.fn()
    const openPane = vi.fn()
    await act(async () => {
      renderHook(() => useIslandOpens('prj_a', setProject, openPane, () => {}), { wrapper: StrictMode })
    })
    expect(openPane).not.toHaveBeenCalled()
    await act(async () => {
      landed()
    })
    expect(setProject).toHaveBeenCalledTimes(1)
    expect(openPane).toHaveBeenCalledTimes(1)
    expect(openPane).toHaveBeenCalledWith('leaf_1')
  })
})
