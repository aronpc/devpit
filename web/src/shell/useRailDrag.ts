import { useEffect, useRef, useState } from 'react'

/*
 * Moving things in the rail by dragging them, with the pointer rather than
 * HTML drag-and-drop.
 *
 * WebKitGTK's drag-and-drop never started a drag here — the rows took the
 * press, showed nothing, and dropped nothing — so every move the rail offered
 * was a move nobody could make. Pointer events are the same on every engine:
 * a press, a few pixels of travel to tell a drag from a click, and the row
 * under the pointer as the target, read from the DOM (`data-drop`).
 */

export type Grab = { kind: 'project'; id: string } | { kind: 'group'; name: string } | { kind: 'orchestrator'; id: string }

/** The row a drop would land on, and on which side of it. */
export interface Spot {
  readonly kind: 'project' | 'group' | 'orchestrator'
  readonly key: string
  readonly after: boolean
}

const KINDS: readonly Spot['kind'][] = ['project', 'group', 'orchestrator']

/**
 * The spot a row under the pointer at `y` offers, or null when the row is not
 * one the rail drops on: an unknown `data-drop` is nobody's row, not a project.
 */
export function spotOf(row: HTMLElement, y: number): Spot | null {
  const kind = KINDS.find((one) => one === row.dataset.drop)
  const key = row.dataset.key
  if (!kind || !key) return null
  const box = row.getBoundingClientRect()
  return { kind, key, after: y > box.top + box.height / 2 }
}

/*
 * Whether a grab may land on a spot. Orchestrators keep to themselves:
 * one lands only on another, and projects and groups never land on one.
 * The rule lives here, not in each list's drop, because every rail list's
 * hook listens on the same window and sees the others' rows.
 */
export function lands(grab: Grab, spot: Spot): boolean {
  return (grab.kind === 'orchestrator') === (spot.kind === 'orchestrator')
}

/* Travel before a press becomes a drag: less and a shaky click drags. */
const SLOP = 5
/* Near the list's top or bottom edge, a drag scrolls it. */
const EDGE = 28

export interface RailDrag {
  readonly grab: Grab | null
  readonly spot: Spot | null
  press: (grab: Grab) => (event: React.PointerEvent) => void
  /** False for the click a drag ends with: it was a drop, not a click. */
  clicked: () => boolean
}

export function useRailDrag(list: React.RefObject<HTMLElement | null>, onDrop: (grab: Grab, spot: Spot) => void): RailDrag {
  const [grab, setGrab] = useState<Grab | null>(null)
  const [spot, setSpot] = useState<Spot | null>(null)
  const pressed = useRef<{ grab: Grab; x: number; y: number } | null>(null)
  const dropped = useRef(false)
  const drop = useRef(onDrop)
  drop.current = onDrop

  useEffect(() => {
    let active: Grab | null = null
    let at: Spot | null = null

    const end = (): void => {
      pressed.current = null
      active = null
      at = null
      document.body.style.removeProperty('cursor')
      setGrab(null)
      setSpot(null)
    }

    const move = (event: PointerEvent): void => {
      const press = pressed.current
      if (!press) return
      if (!active) {
        if (Math.hypot(event.clientX - press.x, event.clientY - press.y) < SLOP) return
        active = press.grab
        document.body.style.cursor = 'grabbing'
        setGrab(active)
      }
      event.preventDefault()
      const row = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>('[data-drop]')
      const next = row ? spotOf(row, event.clientY) : null
      if (next && !lands(active, next)) {
        /* A row it may not land on: no mark left on the last one, no drop. */
        if (at) {
          at = null
          setSpot(null)
        }
      } else if (next && (next.kind !== at?.kind || next.key !== at.key || next.after !== at.after)) {
        at = next
        setSpot(next)
      }
      const scroller = list.current
      if (scroller) {
        const box = scroller.getBoundingClientRect()
        if (event.clientY < box.top + EDGE) scroller.scrollTop -= 8
        else if (event.clientY > box.bottom - EDGE) scroller.scrollTop += 8
      }
    }

    const up = (): void => {
      if (active) {
        dropped.current = true
        if (at) drop.current(active, at)
        /* The click that follows this release is the drop's, not a click. */
        setTimeout(() => (dropped.current = false), 0)
      }
      end()
    }

    const key = (event: KeyboardEvent): void => {
      if (event.key === 'Escape' && active) end()
    }

    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
    window.addEventListener('pointercancel', end)
    window.addEventListener('keydown', key)
    return () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      window.removeEventListener('pointercancel', end)
      window.removeEventListener('keydown', key)
    }
  }, [list])

  return {
    grab,
    spot,
    press: (one) => (event) => {
      if (event.button !== 0) return
      pressed.current = { grab: one, x: event.clientX, y: event.clientY }
    },
    clicked: () => !dropped.current,
  }
}
