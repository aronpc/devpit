import { useRef } from 'react'

import { t } from '../i18n'
import { dragged, held, LEAST, MOST, WIDE, type Panel } from './sizing'

/*
 * The edge between a side panel and the work.
 *
 * A `separator` and not a decoration: it is in the tab order, it says how wide
 * it is, and the arrow keys move it. A divider you can only reach with a mouse
 * is a window somebody cannot arrange without one — and this one is four
 * pixels wide, which is a hard target with a trackpad and an impossible one
 * without a pointer at all.
 *
 * The pointer is captured on the way down, so a drag that leaves the element —
 * which every drag does, immediately — keeps arriving here rather than being
 * delivered to whatever is underneath.
 */

/** How far one arrow press moves the edge. Shift makes it a coarse one. */
const STEP = 8
const LEAP = 48

export function Grip({
  panel,
  width,
  other,
  onSize,
  onDone,
}: {
  panel: Panel
  width: number
  /** What the panel on the far side is taking, which is room this one has
   *  not got. */
  other: number
  onSize: (width: number) => void
  /** Called once, when the edge is let go — never during the drag. */
  onDone: (width: number) => void
}): React.JSX.Element {
  const began = useRef<{ at: number; was: number } | null>(null)

  const put = (wanted: number): number => {
    const next = held(panel, wanted, window.innerWidth, other)
    onSize(next)
    return next
  }

  const nudge = (by: number): void => onDone(put(width + by))

  return (
    <div
      className="seam"
      data-panel={panel}
      role="separator"
      tabIndex={0}
      aria-orientation="vertical"
      aria-label={panel === 'sidebar' ? t('workspace.split.resizeSidebar') : t('workspace.split.resizeSidePanel')}
      aria-valuenow={width}
      aria-valuemin={LEAST[panel]}
      aria-valuemax={MOST[panel]}
      onPointerDown={(event) => {
        if (event.button !== 0) return
        began.current = { at: event.clientX, was: width }
        event.currentTarget.setPointerCapture(event.pointerId)
      }}
      onPointerMove={(event) => {
        const from = began.current
        if (!from) return
        put(dragged(panel, from.at, from.was, event.clientX))
      }}
      onLostPointerCapture={() => {
        if (!began.current) return
        began.current = null
        onDone(width)
      }}
      onDoubleClick={() => onDone(put(WIDE[panel]))}
      onKeyDown={(event) => {
        const by = event.shiftKey ? LEAP : STEP
        // Left and right, and never up and down: this edge only moves one way.
        if (event.key === 'ArrowLeft') nudge(panel === 'sidebar' ? -by : by)
        else if (event.key === 'ArrowRight') nudge(panel === 'sidebar' ? by : -by)
        else return
        event.preventDefault()
      }}
    />
  )
}
