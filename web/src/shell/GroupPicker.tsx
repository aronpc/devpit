import { useEffect, useRef, useState } from 'react'

import { t } from '../i18n'
import { ChevronDown, Plus } from './GitIcons'
import { abandoned, committed } from './typing'

/*
 * The group a project is listed under: none, one that exists, or a new one.
 *
 * Not a text field with suggestions: the browser drew those as a bare white
 * box under the field, and a group is a choice among a few names far more
 * often than a name to type. A new one is typed in the list itself.
 */

export function GroupPicker({
  value,
  groups,
  onChange,
}: {
  value: string
  groups: readonly string[]
  onChange: (group: string) => void
}): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const [typing, setTyping] = useState<string | null>(null)
  const box = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const outside = (event: MouseEvent): void => {
      if (!box.current?.contains(event.target as Node)) {
        setOpen(false)
        setTyping(null)
      }
    }
    document.addEventListener('mousedown', outside, true)
    return () => document.removeEventListener('mousedown', outside, true)
  }, [open])

  const pick = (group: string): void => {
    onChange(group)
    setOpen(false)
    setTyping(null)
  }

  return (
    <div className="gpick" ref={box}>
      <button className="gpick__b" aria-haspopup="listbox" aria-expanded={open} onClick={() => setOpen((was) => !was)}>
        <span className={value ? 'gpick__v' : 'gpick__v gpick__v--none'}>{value || t('workspace.groupPicker.none')}</span>
        <ChevronDown />
      </button>
      {open && (
        <div className="gpick__list" role="listbox">
          <button className="gpick__o" role="option" aria-selected={!value} onClick={() => pick('')}>
            {t('workspace.groupPicker.none')}
          </button>
          {groups.map((group) => (
            <button key={group} className="gpick__o" role="option" aria-selected={group === value} onClick={() => pick(group)}>
              {group}
            </button>
          ))}
          <div className="gpick__rule" />
          {typing === null ? (
            <button className="gpick__o gpick__o--new" onClick={() => setTyping('')}>
              <Plus size={12} />
              {t('workspace.groupPicker.new')}
            </button>
          ) : (
            <input
              className="gpick__new"
              autoFocus
              placeholder={t('workspace.groupPicker.name')}
              value={typing}
              onChange={(event) => setTyping(event.target.value)}
              onKeyDown={(event) => {
                if (committed(event) && typing.trim()) pick(typing.trim())
                if (abandoned(event)) {
                  event.stopPropagation()
                  setTyping(null)
                }
              }}
            />
          )}
        </div>
      )}
    </div>
  )
}
