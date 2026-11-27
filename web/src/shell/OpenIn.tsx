import { useRef, useState } from 'react'

import { t } from '../i18n'
import type { OpenApp } from '../gen/bindings'
import { useAway } from './away'
import { ask, commands } from './live'
import { went } from './problems'
import { Icon } from './Icon'

/*
 * "Open in", where a folder is.
 *
 * Absent rather than empty when nothing is configured: a button that opens a
 * menu with nothing in it is worse than no button, because it looks like the
 * feature is broken rather than unused. One app configured is not a menu
 * either — it is one button that opens it.
 */

export function OpenIn({
  apps,
  path,
}: {
  apps: readonly OpenApp[]
  path: string
}): React.JSX.Element | null {
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  useAway(box, () => setOpen(false), open)

  const usable = apps.filter((app) => app.installed)
  if (usable.length === 0) return null

  const hand = (app: OpenApp): void => {
    setOpen(false)
    void ask(() => commands.appsOpen(app.id, path)).then(went)
  }

  if (usable.length === 1) {
    const only = usable[0]!
    return (
      <button className="btn" onClick={() => hand(only)}>
        {t('workspace.openIn.only', { name: only.label })}
      </button>
    )
  }

  return (
    <div className="opin" ref={box}>
      <button className="btn" aria-expanded={open} onClick={() => setOpen((was) => !was)}>
        {t('workspace.openIn.title')}
        <Icon size="s"><path d="m6 9 6 6 6-6" /></Icon>
      </button>
      {open && (
        <div className="apps__menu" role="menu">
          {usable.map((app) => (
            <button className="apps__opt" key={app.id} role="menuitem" onClick={() => hand(app)}>
              {app.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
