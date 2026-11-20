import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { lands, spotOf, useRailDrag, type Grab, type Spot } from './useRailDrag'

afterEach(cleanup)

function Rail({
  onDrop,
  onClick,
  onSpot,
}: {
  onDrop: (grab: Grab, spot: Spot) => void
  onClick: () => void
  onSpot?: (spot: Spot | null) => void
}): React.JSX.Element {
  const list = useRef<HTMLDivElement>(null)
  const drag = useRailDrag(list, onDrop)
  onSpot?.(drag.spot)
  return (
    <div ref={list}>
      <button data-drop="project" data-key="a" onPointerDown={drag.press({ kind: 'project', id: 'a' })} onClick={() => drag.clicked() && onClick()}>
        a
      </button>
      <div data-drop="group" data-key="Work">
        Work
      </div>
      <button data-drop="orchestrator" data-key="o1" onPointerDown={drag.press({ kind: 'orchestrator', id: 'o1' })}>
        o1
      </button>
      <div data-drop="orchestrator" data-key="o2">
        o2
      </div>
    </div>
  )
}

/* A row 20px tall whose top is at `top`. */
const at = (row: HTMLElement, top: number): HTMLElement => {
  row.getBoundingClientRect = () => ({ top, height: 20, bottom: top + 20, left: 0, right: 0, width: 0, x: 0, y: top, toJSON: () => ({}) })
  return row
}

const pointer = (type: string, x: number, y: number): Event =>
  Object.assign(new Event(type, { bubbles: true, cancelable: true }), { clientX: x, clientY: y, button: 0 })

describe('dragging in the rail', () => {
  it('drops a project on the row under the pointer, on the side the pointer is', () => {
    const onDrop = vi.fn()
    const onClick = vi.fn()
    const { getByText } = render(<Rail onDrop={onDrop} onClick={onClick} />)
    const heading = getByText('Work')
    heading.getBoundingClientRect = () => ({ top: 100, height: 20, bottom: 120, left: 0, right: 0, width: 0, x: 0, y: 100, toJSON: () => ({}) })
    document.elementFromPoint = () => heading

    fireEvent.pointerDown(getByText('a'), { button: 0, clientX: 10, clientY: 10 })
    act(() => {
      window.dispatchEvent(pointer('pointermove', 10, 60))
      window.dispatchEvent(pointer('pointermove', 10, 115))
      window.dispatchEvent(pointer('pointerup', 10, 115))
    })
    expect(onDrop).toHaveBeenCalledWith({ kind: 'project', id: 'a' }, { kind: 'group', key: 'Work', after: true })
    // The click the release makes is the drop's.
    fireEvent.click(getByText('a'))
    expect(onClick).not.toHaveBeenCalled()
  })

  it('is still a click when the pointer barely moved', () => {
    const onDrop = vi.fn()
    const onClick = vi.fn()
    const { getByText } = render(<Rail onDrop={onDrop} onClick={onClick} />)
    fireEvent.pointerDown(getByText('a'), { button: 0, clientX: 10, clientY: 10 })
    act(() => {
      window.dispatchEvent(pointer('pointermove', 12, 11))
      window.dispatchEvent(pointer('pointerup', 12, 11))
    })
    fireEvent.click(getByText('a'))
    expect(onDrop).not.toHaveBeenCalled()
    expect(onClick).toHaveBeenCalledOnce()
  })

  it('drops an orchestrator on another orchestrator, on the side the pointer is', () => {
    const onDrop = vi.fn()
    const { getByText } = render(<Rail onDrop={onDrop} onClick={vi.fn()} />)
    const other = at(getByText('o2'), 200)
    document.elementFromPoint = () => other

    fireEvent.pointerDown(getByText('o1'), { button: 0, clientX: 10, clientY: 10 })
    act(() => {
      window.dispatchEvent(pointer('pointermove', 10, 60))
      window.dispatchEvent(pointer('pointermove', 10, 203))
      window.dispatchEvent(pointer('pointerup', 10, 203))
    })
    expect(onDrop).toHaveBeenCalledWith({ kind: 'orchestrator', id: 'o1' }, { kind: 'orchestrator', key: 'o2', after: false })
  })

  it('refuses an orchestrator over a project row', () => {
    const onDrop = vi.fn()
    const { getByText } = render(<Rail onDrop={onDrop} onClick={vi.fn()} />)
    const project = at(getByText('a'), 100)
    document.elementFromPoint = () => project

    fireEvent.pointerDown(getByText('o1'), { button: 0, clientX: 10, clientY: 10 })
    act(() => {
      window.dispatchEvent(pointer('pointermove', 10, 60))
      window.dispatchEvent(pointer('pointermove', 10, 115))
      window.dispatchEvent(pointer('pointerup', 10, 115))
    })
    expect(onDrop).not.toHaveBeenCalled()
  })

  it('refuses a project over an orchestrator row and lets go of the spot it had', () => {
    const onDrop = vi.fn()
    const spots: (Spot | null)[] = []
    const { getByText } = render(<Rail onDrop={onDrop} onClick={vi.fn()} onSpot={(one) => spots.push(one)} />)
    const heading = at(getByText('Work'), 100)
    const orchestrator = at(getByText('o2'), 200)
    let under: HTMLElement = heading
    document.elementFromPoint = () => under

    fireEvent.pointerDown(getByText('a'), { button: 0, clientX: 10, clientY: 10 })
    act(() => {
      window.dispatchEvent(pointer('pointermove', 10, 115))
    })
    expect(spots.at(-1)).toEqual({ kind: 'group', key: 'Work', after: true })
    under = orchestrator
    act(() => {
      window.dispatchEvent(pointer('pointermove', 10, 215))
    })
    expect(spots.at(-1)).toBeNull()
    act(() => {
      window.dispatchEvent(pointer('pointerup', 10, 215))
    })
    expect(onDrop).not.toHaveBeenCalled()
  })
})

describe('where a drag may land', () => {
  const project: Grab = { kind: 'project', id: 'a' }
  const group: Grab = { kind: 'group', name: 'Work' }
  const orchestrator: Grab = { kind: 'orchestrator', id: 'o1' }
  const on = (kind: Spot['kind']): Spot => ({ kind, key: 'k', after: false })

  it('lands lets an orchestrator land only on an orchestrator', () => {
    expect(lands(orchestrator, on('orchestrator'))).toBe(true)
    expect(lands(orchestrator, on('project'))).toBe(false)
    expect(lands(orchestrator, on('group'))).toBe(false)
  })

  it('lands never lets a project or a group land on an orchestrator', () => {
    expect(lands(project, on('orchestrator'))).toBe(false)
    expect(lands(group, on('orchestrator'))).toBe(false)
    expect(lands(project, on('project'))).toBe(true)
    expect(lands(project, on('group'))).toBe(true)
    expect(lands(group, on('project'))).toBe(true)
    expect(lands(group, on('group'))).toBe(true)
  })

  it('spotOf reads project, group and orchestrator rows and ignores any other data-drop', () => {
    const row = (drop: string): HTMLElement => {
      const one = document.createElement('div')
      one.dataset.drop = drop
      one.dataset.key = 'k'
      return at(one, 100)
    }
    expect(spotOf(row('project'), 115)).toEqual({ kind: 'project', key: 'k', after: true })
    expect(spotOf(row('group'), 105)).toEqual({ kind: 'group', key: 'k', after: false })
    expect(spotOf(row('orchestrator'), 115)).toEqual({ kind: 'orchestrator', key: 'k', after: true })
    expect(spotOf(row('session'), 115)).toBeNull()
    expect(spotOf(row(''), 115)).toBeNull()
  })
})
