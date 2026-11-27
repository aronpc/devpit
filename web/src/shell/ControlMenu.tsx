import { useCallback, useRef, useState } from 'react'

import { useAway } from './away'
import { Icon } from './Icon'

/*
 * One chip that opens a list of choices.
 *
 * A `<select>` was here and it drew the operating system's own dropdown over
 * the window — unstyled, unplaceable, and unable to show the one thing that
 * matters: what each choice actually does.
 */

export interface Choice {
  readonly id: string
  readonly label: string
  /** What it does. A label alone often names nothing. */
  readonly what?: string
  /** A quiet word after the label — "Default", say. */
  readonly suffix?: string
  readonly section?: string
  readonly selected?: boolean
}

export function ControlMenu({
  label,
  title,
  choices,
  onPick,
  opens = 'up',
}: {
  label: string
  title: string
  choices: readonly Choice[]
  onPick: (id: string) => void
  /** Up over a composer; down from a bar at the top, where up is off the pane. */
  opens?: 'up' | 'down'
}): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  useAway(box, useCallback(() => setOpen(false), []), open)

  return (
    <div className="ctl" ref={box}>
      <button
        className="chip"
        aria-label={title}
        aria-expanded={open}
        onClick={() => setOpen((was) => !was)}
      >
        <span className="ctl__l">{label}</span>
        <Icon size="s" className="ctl__v"><path d="m6 9 6 6 6-6" /></Icon>
      </button>

      {open && (
        <div className={opens === 'down' ? 'ctlmenu ctlmenu--down' : 'ctlmenu'} role="menu" aria-label={title}>
          {choices.map((choice, at) => (
            <div key={choice.id}>
              {choice.section && choices[at - 1]?.section !== choice.section && (
                <div className="ctlmenu__s">{choice.section}</div>
              )}
              <button
                className="ctlmenu__i"
                role="menuitemradio"
                aria-checked={choice.selected ?? false}
                onClick={() => {
                  setOpen(false)
                  onPick(choice.id)
                }}
              >
                <span className="ctlmenu__b">
                  <span className="ctlmenu__n">
                    {choice.label}
                    {choice.suffix && <span className="ctlmenu__x">{choice.suffix}</span>}
                  </span>
                  {choice.what && <span className="ctlmenu__w">{choice.what}</span>}
                </span>
                {choice.selected && (
                  <Icon size="s" className="ctlmenu__c"><path d="m5 13 4 4L19 7" /></Icon>
                )}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
