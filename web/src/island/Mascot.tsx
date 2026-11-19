import { useEffect, useRef, useState } from 'react'

import asking from '../assets/brand/island/asking.png'
import done from '../assets/brand/island/done.png'
import failed from '../assets/brand/island/failed.png'
import sleeping from '../assets/brand/island/sleeping.png'
import thinking from '../assets/brand/island/thinking.png'
import waiting from '../assets/brand/island/waiting.png'
import working from '../assets/brand/island/working.png'
import mark from '../assets/brand/mark.png'
import { t } from '../i18n'
import { onCarried } from '../shell/window'
import type { Mood } from './sessions'

/*
 * devpit's face on the island: the brand's own picture, made to move.
 *
 * A picture per mood, the same face drawn waiting, asking, thinking, done,
 * and moved by CSS on top — breathing, a bob while it works, a tilt while it
 * waits on you, a shake when it asks. A mood without its own picture keeps
 * the brand's.
 *
 * It looks at the cursor by leaning toward it, written straight onto the
 * element's style: thirty cursor events a second would otherwise be thirty
 * renders of the whole island.
 */

export const FACES: Partial<Record<Mood, string>> = { asking, done, failed, sleeping, thinking, waiting, working }

/** What the badge says for each mood; a mood with none shows none. */
export const BADGES: Partial<Record<Mood, string>> = {
  thinking: '•••',
  waiting: '!',
  asking: '?',
  done: '✓',
  failed: '×',
  sleeping: 'z',
}

/** How much the face leans toward the cursor, from −1 to 1 on each axis. */
export function leaning(face: { x: number; y: number }, cursor: { x: number; y: number }): { x: number; y: number } {
  return { x: Math.tanh((cursor.x - face.x) / 260), y: Math.tanh((cursor.y - face.y) / 200) }
}

interface Cursor {
  readonly x: number
  readonly y: number
}

export function Mascot({
  mood,
  size,
  color,
  looks = false,
  label,
}: {
  mood: Mood
  size: number
  /** The ring's colour, when it is one project's face. */
  color?: string | null
  /** Whether it follows the cursor: the big face does, the small ones do not. */
  looks?: boolean
  label?: string
}): React.JSX.Element {
  const face = useRef<HTMLSpanElement>(null)
  const [react, setReact] = useState<'boop' | 'dizzy' | null>(null)
  const taps = useRef<number[]>([])

  useEffect(() => {
    if (!looks) return
    const lean = (cursor: { x: number; y: number }): void => {
      const at = face.current
      if (!at) return
      const box = at.getBoundingClientRect()
      const leaned = leaning({ x: box.left + box.width / 2, y: box.top + box.height / 2 }, cursor)
      at.style.setProperty('--lx', leaned.x.toFixed(3))
      at.style.setProperty('--ly', leaned.y.toFixed(3))
    }
    /* The page's own pointer as well: on Wayland nothing can say where the
       cursor is outside the window, so only the moves over it are known. */
    const moved = (event: PointerEvent): void => lean({ x: event.clientX, y: event.clientY })
    window.addEventListener('pointermove', moved)
    const stop = onCarried<Cursor>('island:cursor', lean)
    return () => {
      window.removeEventListener('pointermove', moved)
      stop()
    }
  }, [looks])

  useEffect(() => {
    if (!react) return
    const done = window.setTimeout(() => setReact(null), react === 'dizzy' ? 2400 : 420)
    return () => window.clearTimeout(done)
  }, [react])

  /* A tap squishes it; three quick ones make it dizzy. */
  const tapped = (event: React.MouseEvent): void => {
    event.stopPropagation()
    const now = Date.now()
    taps.current = [...taps.current.filter((at) => now - at < 900), now]
    setReact(taps.current.length >= 3 ? 'dizzy' : 'boop')
    if (taps.current.length >= 3) taps.current = []
  }

  /* Too small to read below this, so a small face says it with its ring. */
  const badge = size >= 40 ? BADGES[mood] : undefined
  return (
    <span
      ref={face}
      className="isl-face"
      data-mood={mood}
      data-react={react ?? undefined}
      style={{ '--size': `${size}px`, '--ring': color ?? 'var(--accent)' } as React.CSSProperties}
      role="img"
      aria-label={label ?? t('island.mascot.label', { mood })}
      onClick={looks ? tapped : undefined}
    >
      <span className="isl-face__ring" />
      <span className="isl-face__lean">
        <span className="isl-face__body">
          <img
            className="isl-face__img"
            src={FACES[mood] ?? mark}
            alt="" // ui-text: ok decorative image, the empty alt is the point
            draggable={false}
          />
          <span className="isl-face__shine" />
        </span>
      </span>
      {!FACES[mood] && <span className="isl-face__mic" />}
      {badge && <span className="isl-face__badge">{badge}</span>}
    </span>
  )
}
