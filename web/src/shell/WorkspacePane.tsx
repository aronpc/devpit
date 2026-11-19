import { useEffect, useState } from 'react'

import type { Held } from '../gen/bindings'
import { bytes } from './disk'
import { ask, commands } from './live'
import { useShell } from './useShell'
import { t } from '../i18n'

/*
 * What the workspace is holding, measured now.
 *
 * The rows used to be fixed, with sizes nobody read. Worktrees and transcripts
 * grow without announcing themselves, so the number has to come off the disk
 * or it is worse than absent.
 */

export function WorkspacePane(): React.JSX.Element {
  const { project } = useShell()
  const [held, setHeld] = useState<readonly Held[]>([])
  const [directory, setDirectory] = useState('')
  const [total, setTotal] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void ask(() => commands.workspaceRead(project?.id ?? null)).then((answer) => {
      setHeld(answer.data?.held ?? [])
      setDirectory(answer.data?.directory ?? '')
      setTotal(answer.data?.bytes ?? null)
      setError(answer.error)
    })
  }, [project])

  const room = bytes(total)

  return (
    <>
      <div className="pane__bar">
        <span className="pane__t">
          <b>{t('workspace.workspacePane.title')}</b>
          {room ? ` · ${room}` : ''}
        </span>
        <span className="drag"></span>
        <button
          className="btn"
          onClick={() => void ask(() => commands.pathReveal(directory))}
          disabled={!directory}
        >
          {t('workspace.workspacePane.reveal')}
        </button>
      </div>

      <div className="scroll">
        <div className="thread">
          {error && <div className="exempty__t">{error}</div>}
          <div className="wsrow__w">{directory}</div>
          {held.map((row) => (
            <button
              className="wsrow"
              key={row.name}
              onClick={() => void ask(() => commands.pathOpen(row.path))}
              disabled={!row.exists}
              title={row.path}
            >
              <span className="wsrow__n">{row.name}</span>
              <span className="wsrow__w">
                {row.exists
                  ? [bytes(row.bytes), row.count !== null ? t('workspace.workspacePane.items', { count: row.count }) : null]
                      .filter(Boolean)
                      .join(' · ') || t('workspace.workspacePane.empty')
                  : t('workspace.workspacePane.notMade')}
              </span>
            </button>
          ))}
        </div>
      </div>
    </>
  )
}
