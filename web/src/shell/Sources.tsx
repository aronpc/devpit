import { useCallback, useEffect, useState } from 'react'

import type { Source } from '../gen/bindings'
import { ask, commands } from './live'
import { useShell } from './useShell'
import { t } from '../i18n'

/*
 * Which kinds of checkout the lists are about.
 *
 * `git worktree list` returns every worktree of the repository, and somebody
 * working with agents has three kinds at once: devpit's card checkouts, the
 * ones Claude Code makes for itself, and whatever they made by hand. Added up,
 * the project row said "7 worktrees" about a repository they had made none of.
 *
 * Hiding is a view, never a deletion. The count stays on the row while it is
 * hidden, which is the only honest way to offer the switch: it says what the
 * switch is taking away rather than making it disappear and mean nothing.
 */

export function Sources(): React.JSX.Element {
  const { project, reloadProjects } = useShell()
  const [rows, setRows] = useState<readonly Source[]>([])

  const load = useCallback(() => {
    void ask(() => commands.worktreeSources(project?.id ?? null)).then((answer) =>
      setRows(answer.data ?? []),
    )
  }, [project])

  useEffect(load, [load])

  const show = (id: string, shown: boolean): void => {
    void ask(() => commands.worktreeSourceShow(project?.id ?? null, id, shown)).then((answer) => {
      if (!answer.data) return
      setRows(answer.data)
      /* The project row counts the same worktrees this hides, so it has to
         hear about it — otherwise the sidebar keeps the old number until
         something else happens to reload the list. */
      reloadProjects()
    })
  }

  if (rows.length === 0) return <></>

  return (
    <section className="src">
      <div className="src__head">
        <span className="pref__t">{t('workspace.sources.title')}</span>
        <span className="pref__d">
          {t('workspace.sources.hint')}
        </span>
      </div>

      {rows.map((row) => (
        <div className="src__r" key={row.id}>
          <span className="src__b">
            <span className="src__t">{row.label}</span>
            <span className="src__h">{row.hint}</span>
          </span>
          <span className="src__n">{row.count}</span>
          <span className="src__sw" role="radiogroup" aria-label={row.label}>
            <button
              className="src__o"
              role="radio"
              aria-checked={row.shown}
              onClick={() => show(row.id, true)}
            >
              {t('workspace.sources.show')}
            </button>
            <button
              className="src__o"
              role="radio"
              aria-checked={!row.shown}
              onClick={() => show(row.id, false)}
            >
              {t('workspace.sources.hide')}
            </button>
          </span>
        </div>
      ))}
    </section>
  )
}
