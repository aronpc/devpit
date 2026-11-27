import { useState } from 'react'

import { t } from '../i18n'
import { ask, commands } from './live'
import { OpenIn } from './OpenIn'
import { remote, since } from './projects'
import { Rename } from './Rename'
import { useOpeners } from './useOpeners'
import { useShell } from './useShell'
import { Icon } from './Icon'

/*
 * One row per project, showing what the backend already knew.
 *
 * The row drew a folder, a name, a path and an empty 92px column. Everything
 * it needed was in the project table and never left it: when it was last
 * opened — the very column the list is ordered by — and the remote that says
 * whether two rows are the same repository twice.
 *
 * A project whose git cannot be read keeps its path. The old row replaced the
 * path with the error, which took away the one thing you need to go and fix
 * it.
 */

const Folder = (): React.JSX.Element => (
  <Icon size="m" className="pj__ico">
    <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />
  </Icon>
)

export function ProjectRows({ onRemove }: { onRemove: (id: string) => void }): React.JSX.Element {
  const { projects, project, setProject, projectsError, renameProject } = useShell()
  const openers = useOpeners()
  const [naming, setNaming] = useState<string | null>(null)
  /* Why a rename was refused, against the row that asked: an empty name comes
     back as an error, and a field that clears itself says nothing. */
  const [refused, setRefused] = useState<string | null>(null)

  if (projectsError) return <p className="acc__note">{projectsError}</p>
  if (projects.length === 0) return <p className="acc__note">{t('workspace.projects.none')}</p>

  const rename = (id: string, name: string | null): void => {
    setNaming(null)
    if (!name) return
    void renameProject(id, name).then(setRefused)
  }

  return (
    <>
      {projects.map((row) => {
        const here = row.id === project?.id
        const origin = remote(row.origin)
        return (
          <div className="pj" key={row.id} data-here={here}>
            <Folder />
            <span className="pj__b">
              <span className="pj__n">
                <Rename
                  value={row.name}
                  editing={naming === row.id}
                  onDone={(name) => rename(row.id, name)}
                />
                {here && <span className="pj__here">{t('workspace.projects.open')}</span>}
              </span>
              <span className="pj__p">
                <span className="pj__path" title={row.rootPath}>{row.rootPath}</span>
                {origin && <span className="pj__remote" title={row.origin ?? ''}>{origin}</span>}
              </span>
            </span>

            <span className="pj__meta">
              <span className="pj__when">{since(row.lastOpenedAt)}</span>
              <span className="pj__live" data-live={row.worktrees.length}>
                {t('workspace.projects.worktrees', { count: row.worktrees.length })}
              </span>
            </span>

            <span className="pj__acts">
              {!here && (
                <button className="btn" onClick={() => setProject(row.id)}>
                  {t('workspace.projects.open')}
                </button>
              )}
              <button className="btn" onClick={() => setNaming(row.id)}>
                {t('workspace.projects.rename')}
              </button>
              {/* The folder, in whatever this desktop uses for folders. The
                  path is right there and copying it by hand is the step this
                  removes. */}
              <button className="btn" onClick={() => void ask(() => commands.pathReveal(row.rootPath))}>
                {t('workspace.projects.reveal')}
              </button>
              <OpenIn apps={openers} path={row.rootPath} />
              <button className="btn" data-danger onClick={() => onRemove(row.id)}>
                {t('workspace.projects.remove')}
              </button>
            </span>

            {row.unreadable && (
              <span className="pj__warn">
                <Icon size="s">
                  <path d="M12 9v4M12 17v.01" />
                  <path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" />
                </Icon>
                {row.unreadable}
              </span>
            )}
          </div>
        )
      })}
      {refused && <p className="acc__note">{refused}</p>}
    </>
  )
}
