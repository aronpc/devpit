import { useCallback, useEffect, useRef, useState } from 'react'

import type { HostLink } from '../gen/bindings'
import { t } from '../i18n'
import { useAway } from './away'
import { ask, commands } from './live'
import { went } from './problems'
import { useShell } from './useShell'

/*
 * A new terminal on a host the project is linked to: a tab whose shell runs
 * there, in the project's folder. Nothing shows for a project on no host.
 */

export function HostTerminals(): React.JSX.Element | null {
  const { project } = useShell()
  const [linked, setLinked] = useState<{ id: string; links: readonly HostLink[] } | null>(null)
  const [open, setOpen] = useState(false)
  const [opening, setOpening] = useState<string | null>(null)
  const box = useRef<HTMLSpanElement>(null)
  const id = project?.id ?? null
  useAway(box, useCallback(() => setOpen(false), []), open)

  useEffect(() => {
    if (!id) return
    void ask(() => commands.projectHosts(id)).then((answer) => setLinked({ id, links: answer.data?.links ?? [] }))
  }, [id])

  /* Another project's links never show while this one's are asked. */
  const links = linked?.id === id ? linked.links : []
  if (!id || links.length === 0) return null
  const start = (link: HostLink): void => {
    setOpen(false)
    setOpening(link.hostName)
    void ask(() => commands.hostTerminalOpen(id, link.hostId)).then((answer) => {
      setOpening(null)
      went(answer)
    })
  }
  return (
    <span className="hostterm" ref={box}>
      <button
        className="sq26"
        aria-label={t('workspace.hostTerminals.label')}
        title={opening ? t('workspace.hostTerminals.opening', { name: opening }) : t('workspace.hostTerminals.label')}
        aria-expanded={open}
        aria-busy={opening !== null}
        disabled={opening !== null}
        onClick={() => setOpen(!open)}
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="4" width="18" height="7" rx="1" /><rect x="3" y="13" width="18" height="7" rx="1" /><path d="M7 7.5h.01M7 16.5h.01" /></svg>
      </button>
      {open && (
        <div className="hostterm__menu" role="menu">
          {links.map((link) => (
            <button key={link.hostId} role="menuitem" onClick={() => start(link)}>
              {t('workspace.hostTerminals.new', { name: link.hostName })} <span className={`host__env host__env--${link.environment}`}>{link.environment}</span>
            </button>
          ))}
        </div>
      )}
    </span>
  )
}
