import { useCallback, useEffect, useState } from 'react'

import type { CardWorktree } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { Confirm } from './Confirm'
import { bytes } from './disk'
import { ask, commands } from './live'
import { useShell } from './useShell'
import { ScopedSetting } from './ScopedSetting'
import { Sources } from './Sources'
import { WorktreeBase } from './WorktreeBase'
import { WorktreeBranch } from './WorktreeBranch'
import { WorktreeCleanup } from './WorktreeCleanup'

/*
 * The checkouts the cards of this project have.
 *
 * Nothing here removes anything on its own. A worktree holding uncommitted
 * work is refused by the backend, and the refusal is what fills this dialog —
 * so the number of files and lines comes from git, not from the screen's idea
 * of it.
 */

export function Worktrees(): React.JSX.Element {
  const { project } = useShell()
  const [rows, setRows] = useState<readonly CardWorktree[]>([])
  const [total, setTotal] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [going, setGoing] = useState<CardWorktree | null>(null)

  const reload = useCallback(() => {
    if (!project) return
    void ask(() => commands.worktreeList(project.id)).then((answer) => {
      setRows(answer.data?.worktrees ?? [])
      setTotal(answer.data?.diskBytes ?? null)
      setError(answer.error)
    })
  }, [project])

  useEffect(reload, [reload])

  const remove = (row: CardWorktree, evenDirty: boolean): void => {
    if (!project) return
    void ask(() => commands.worktreeRemove(project.id, row.cardId, evenDirty)).then((answer) => {
      setError(answer.error)
      setGoing(null)
      reload()
    })
  }

  const room = bytes(total)

  return (
    <>
      <div className="prefs__hrow">
        <h1 className="prefs__h">{t('workspace.worktrees.title')}</h1>
        {room && <span className="pref__d">{t('workspace.worktrees.onDisk', { size: room })}</span>}
      </div>

      <WorktreeBase />
      <WorktreeBranch />
      <ScopedSetting field="branchPattern" />
      <Sources />

      <p className="prefs__gt">{t('workspace.worktrees.cleanup')}</p>
      <WorktreeCleanup />

      {error && <p className="pref__d">{error}</p>}
      {rows.length === 0 && !error && (
        <p className="pref__d">{t('workspace.worktrees.none')}</p>
      )}

      {rows.map((row) => (
        <div className="pref" key={row.cardId}>
          <span className="pref__body">
            <span className="pref__t">
              {row.cardTitle ?? row.folder}
              {row.orphan && <span className="pill">{t('workspace.worktrees.orphan')}</span>}
            </span>
            <span className="pref__d">
              {[
                row.branch,
                bytes(row.diskBytes),
                row.uncommittedFiles > 0
                  ? `${t('workspace.worktrees.uncommitted', { count: row.uncommittedFiles })}, ${t('workspace.worktrees.lines', { count: row.uncommittedLines })}`
                  : null,
              ]
                .filter(Boolean)
                .join(' · ')}
            </span>
          </span>
          <button className="btn" onClick={() => setGoing(row)}>
            {t('workspace.worktrees.remove')}
          </button>
        </div>
      ))}

      {going && (
        <Confirm
          title={t('workspace.worktrees.removeTitle', { name: going.cardTitle ?? going.folder })}
          danger={t('workspace.worktrees.remove')}
          body={
            <>
              {tRich('workspace.worktrees.folderGoes', { stays: <b>{t('workspace.worktrees.branchStays')}</b> })}
              {going.uncommittedFiles > 0 && (
                <>
                  {' '}
                  <b>
                    {t('workspace.worktrees.lost', {
                      files: t('workspace.worktrees.files', { count: going.uncommittedFiles }),
                      lines: t('workspace.worktrees.lines', { count: going.uncommittedLines }),
                    })}
                  </b>
                </>
              )}
            </>
          }
          onClose={() => setGoing(null)}
          onConfirm={() => remove(going, going.uncommittedFiles > 0)}
        />
      )}
    </>
  )
}
