import { useCallback, useEffect, useRef, useState } from 'react'

import type { Branch } from '../gen/bindings'
import { t } from '../i18n'
import { useAway } from './away'
import { ask, commands } from './live'
import { useShell } from './useShell'

/*
 * The branch this checkout is on, and moving it.
 *
 * git refuses a switch that would drop uncommitted work, and its refusal is
 * shown as it came: it names the files, and a summary here would name fewer.
 */

/** Past this many, the menu leads with a filter. */
const FILTER_FROM = 8

/** The branches whose name or last subject holds every word typed. */
export function matching(branches: readonly Branch[], filter: string): readonly Branch[] {
  const words = filter.toLowerCase().split(/\s+/).filter(Boolean)
  if (words.length === 0) return branches
  return branches.filter((one) => {
    const text = `${one.name} ${one.subject}`.toLowerCase()
    return words.every((word) => text.includes(word))
  })
}

export function BranchPicker({ branch, ahead }: { branch: string; ahead: number }): React.JSX.Element {
  const { project, reloadProjects } = useShell()
  const [open, setOpen] = useState(false)
  const [branches, setBranches] = useState<readonly Branch[]>([])
  const [refused, setRefused] = useState<string | null>(null)
  const [filter, setFilter] = useState('')
  const box = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open || !project) return
    setFilter('')
    void ask(() => commands.branchList(project.id, null)).then((answer) => {
      setBranches(answer.data?.branches ?? [])
      setRefused(answer.error)
    })
  }, [open, project])

  useAway(box, useCallback(() => setOpen(false), []), open)

  const go = (name: string): void => {
    if (!project) return
    void ask(() => commands.branchSwitch(project.id, null, name)).then((answer) => {
      setRefused(answer.error)
      if (answer.data) {
        setBranches(answer.data.branches)
        setOpen(false)
        /* Files open from the old branch may be different now, and the tree
           certainly is. Reloading the projects is what refreshes both. */
        reloadProjects()
      }
    })
  }

  return (
    <div className="branchbox" ref={box}>
      <button className="branch" onClick={() => setOpen((was) => !was)} title={t('workspace.branchPicker.switch')}>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
          <line x1="6" y1="3" x2="6" y2="15" />
          <circle cx="18" cy="6" r="3" />
          <circle cx="6" cy="18" r="3" />
          <path d="M18 9a9 9 0 0 1-9 9" />
        </svg>
        <span className="branch__n">{branch}</span>
        {ahead > 0 && <span className="branch__ahead">&uarr;{ahead}</span>}
      </button>

      {open && (
        <div className="branchmenu" role="menu">
          {branches.length > FILTER_FROM && (
            <input
              className="branchmenu__f"
              autoFocus
              placeholder={t('workspace.branchPicker.filter')}
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              onKeyDown={(event) => event.key === 'Escape' && setOpen(false)}
            />
          )}
          <div className="branchmenu__list">
          {matching(branches, filter).map((one) => (
            <button
              className="branchmenu__i"
              role="menuitem"
              key={one.name}
              aria-current={one.current}
              onClick={() => go(one.name)}
            >
              <span className="branchmenu__n">{one.name}</span>
              <span className="branchmenu__s">{one.subject}</span>
            </button>
          ))}
          </div>
          {refused && <div className="branchmenu__no">{refused}</div>}
        </div>
      )}
    </div>
  )
}
