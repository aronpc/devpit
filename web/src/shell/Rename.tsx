import { useEffect, useRef, useState } from 'react'

import { t } from '../i18n'
import { abandoned, committed } from './typing'

/*
 * A name you can change in place.
 *
 * In place rather than in a dialog: the name is already on screen, and a
 * dialog that shows you the same string in a different box only adds a step
 * between deciding and typing.
 *
 * Enter commits, Escape puts back what was there, and clicking away commits —
 * because a name you typed and then clicked off is a name you meant.
 */

export function Rename({
  value,
  editing,
  className,
  onDone,
}: {
  value: string
  editing: boolean
  className?: string
  onDone: (title: string | null) => void
}): React.JSX.Element {
  const [draft, setDraft] = useState(value)
  const field = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!editing) return
    setDraft(value)
    /* Focused *and* selected: `select` alone leaves the caret where it was in
       WebKit, so the field would appear and quietly swallow every keystroke.
       The frame is for the mount — there is nothing to focus before it. */
    const frame = requestAnimationFrame(() => {
      field.current?.focus()
      field.current?.select()
    })
    return () => cancelAnimationFrame(frame)
  }, [editing, value])

  if (!editing) return <span className={className}>{value}</span>

  const commit = (): void => {
    const next = draft.trim()
    onDone(next && next !== value ? next : null)
  }

  return (
    <input
      ref={field}
      className={`rename ${className ?? ''}`}
      value={draft}
      spellCheck={false}
      aria-label={t('workspace.rename.name')}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onClick={(event) => event.stopPropagation()}
      onPointerDown={(event) => event.stopPropagation()}
      onKeyDown={(event) => {
        event.stopPropagation()
        if (committed(event)) {
          event.preventDefault()
          commit()
        } else if (abandoned(event)) {
          event.preventDefault()
          onDone(null)
        }
      }}
    />
  )
}
