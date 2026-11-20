import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { Project } from '../gen/bindings'
import { RailOrchestrators } from './RailOrchestrators'
import { savedOrchestrators } from './rail'

const setProject = vi.fn()
let projects: Project[] = []
vi.mock('./useShell', () => ({
  useShell: () => ({ projects, project: null, setProject, show: vi.fn(), open: [], reloadProjects: vi.fn() }),
}))

const orchestrator = (id: string, name: string): Project => ({ id, name, rootPath: `/o/${name}`, orchestrator: 'claude' }) as unknown as Project

/* A box `height` tall (a row, 20px) whose top is at `top`. */
const at = (row: HTMLElement, top: number, height = 20): HTMLElement => {
  row.getBoundingClientRect = () => ({ top, height, bottom: top + height, left: 0, right: 0, width: 0, x: 0, y: top, toJSON: () => ({}) })
  return row
}

const pointer = (type: string, x: number, y: number): Event =>
  Object.assign(new Event(type, { bubbles: true, cancelable: true }), { clientX: x, clientY: y, button: 0 })

const draw = (onHold: (held: boolean) => void = vi.fn()): ReturnType<typeof render> =>
  render(<RailOrchestrators onMenu={vi.fn()} onRemove={vi.fn()} onEdit={vi.fn()} onHold={onHold} />)

const row = (name: string): HTMLElement => screen.getByText(name).closest('button')!
const drawn = (): string[] => [...document.querySelectorAll<HTMLElement>('[data-drop="orchestrator"]')].map((one) => one.dataset.key!)

/* Presses `from`, travels past the slop and lets go over `to` at `y`. */
const dragOnto = (from: HTMLElement, to: HTMLElement, y: number): void => {
  document.elementFromPoint = () => to
  fireEvent.pointerDown(from, { button: 0, clientX: 10, clientY: 10 })
  act(() => {
    window.dispatchEvent(pointer('pointermove', 10, 60))
    window.dispatchEvent(pointer('pointermove', 10, y))
    window.dispatchEvent(pointer('pointerup', 10, y))
  })
}

beforeEach(() => {
  localStorage.clear()
  setProject.mockClear()
  /* Last-opened first, the way the list comes back. */
  projects = [orchestrator('prj_01M3B', 'Second'), orchestrator('prj_01M3A', 'First')]
})
afterEach(cleanup)

describe('dragging an orchestrator in the rail', () => {
  it('draws unplaced orchestrators oldest first, however the list arrives', () => {
    draw()
    expect(drawn()).toEqual(['prj_01M3A', 'prj_01M3B'])
  })

  it('reorders orchestrators by drag and reads the same order on reopen', () => {
    draw()
    dragOnto(row('First'), at(row('Second'), 200), 215)
    expect(drawn()).toEqual(['prj_01M3B', 'prj_01M3A'])
    expect(savedOrchestrators()).toEqual(['prj_01M3B', 'prj_01M3A'])

    cleanup()
    draw()
    expect(drawn()).toEqual(['prj_01M3B', 'prj_01M3A'])
  })

  it('marks orchestrator rows as drop rows inside a scroller of their own', () => {
    draw()
    const first = row('First')
    expect(first.dataset.drop).toBe('orchestrator')
    expect(first.dataset.key).toBe('prj_01M3A')
    const scroller = at(first.parentElement!, 0, 400)
    expect(row('Second').parentElement).toBe(scroller)
    expect(scroller.querySelectorAll('button')).toHaveLength(2)

    const second = at(row('Second'), 200)
    document.elementFromPoint = () => second
    fireEvent.pointerDown(first, { button: 0, clientX: 10, clientY: 10 })
    act(() => {
      window.dispatchEvent(pointer('pointermove', 10, 60))
      window.dispatchEvent(pointer('pointermove', 10, 203))
    })
    expect(row('First').dataset.dragging).toBe('true')
    expect(row('Second').dataset.over).toBe('before')
    expect(scroller.scrollTop).toBe(0)
    /* Near its bottom edge the drag scrolls the orchestrators' own list. */
    act(() => void window.dispatchEvent(pointer('pointermove', 10, 390)))
    expect(scroller.scrollTop).toBe(8)
    act(() => void window.dispatchEvent(pointer('pointerup', 10, 390)))
    expect(row('First').dataset.dragging).toBeUndefined()
    expect(row('Second').dataset.over).toBeUndefined()
  })

  it('opens an orchestrator on a plain click without saving an order', () => {
    draw()
    fireEvent.pointerDown(row('Second'), { button: 0, clientX: 10, clientY: 10 })
    act(() => void window.dispatchEvent(pointer('pointerup', 10, 10)))
    fireEvent.click(row('Second'))
    expect(setProject).toHaveBeenCalledWith('prj_01M3B')
    expect(localStorage.getItem('devpit.rail.orchestrators')).toBeNull()
    expect(drawn()).toEqual(['prj_01M3A', 'prj_01M3B'])
  })

  it('tells the rail to hold open while an orchestrator is dragged, and the drop is no click', () => {
    const onHold = vi.fn()
    draw(onHold)
    expect(onHold).toHaveBeenLastCalledWith(false)
    const second = at(row('Second'), 200)
    document.elementFromPoint = () => second
    fireEvent.pointerDown(row('First'), { button: 0, clientX: 10, clientY: 10 })
    act(() => void window.dispatchEvent(pointer('pointermove', 10, 60)))
    expect(onHold).toHaveBeenLastCalledWith(true)
    act(() => void window.dispatchEvent(pointer('pointerup', 10, 215)))
    expect(onHold).toHaveBeenLastCalledWith(false)
    fireEvent.click(row('First'))
    expect(setProject).not.toHaveBeenCalled()
  })
})
