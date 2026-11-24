import { useCallback, useRef, useState } from 'react'

import { t } from '../i18n'
import { useAway } from './away'
import { ChevronDown } from './GitIcons'

export interface FieldOption {
  readonly id: string
  readonly label: string
  /** A second, quieter line: what the option runs, or why it cannot be picked. */
  readonly hint?: string
}

/* A select that reads as a form field: as wide as the field it sits in, and
   its list opening under it at the same width. The composer's chip menu is
   built to float over a thread, and in a dialog it floated over the buttons. */
export function FieldSelect({
  label,
  value,
  options,
  onPick,
}: {
  label: string
  value: string | null
  options: readonly FieldOption[]
  onPick: (id: string) => void
}): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  useAway(box, useCallback(() => setOpen(false), []), open)
  const chosen = options.find((one) => one.id === value)
  return (
    <div className="fsel" ref={box}>
      <button type="button" className="fsel__b" aria-haspopup="listbox" aria-expanded={open} aria-label={label} onClick={() => setOpen((was) => !was)}>
        <span className="fsel__v">{chosen ? chosen.label : t('common.pickOne')}</span>
        {chosen?.hint && <span className="fsel__h">{chosen.hint}</span>}
        <ChevronDown size={12} />
      </button>
      {open && (
        <div className="fsel__menu" role="listbox" aria-label={label}>
          {options.map((one) => (
            <button
              type="button"
              key={one.id}
              className="fsel__o"
              role="option"
              aria-selected={one.id === value}
              onClick={() => {
                setOpen(false)
                onPick(one.id)
              }}
            >
              <span className="fsel__v">{one.label}</span>
              {one.hint && <span className="fsel__h">{one.hint}</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
