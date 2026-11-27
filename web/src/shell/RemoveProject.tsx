import { useState } from 'react'

import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { useShell } from './useShell'
import { Icon } from './Icon'

/* Where a project's devpit workspace lives; a path, so it is not translated. */
const WORKSPACES = '~/.devpit/projects'

/*
 * "Remove" and "delete" are the same word to most people, and only one of them
 * is true here — so the body says which, and deleting the workspace is a
 * separate box you have to tick.
 */
export function RemoveProject({
  project,
  onClose,
  onConfirm,
}: {
  project: string
  onClose: () => void
  onConfirm: (wipe: boolean) => void
}): React.JSX.Element {
  const [wipe, setWipe] = useState(false)
  const { projects } = useShell()
  const row = projects.find((other) => other.id === project)
  const name = row?.name ?? project
  const live = row?.worktrees.length ?? 0

  return (
    <div
      className="ask"
      data-open="true"
      onClick={(event) => event.target === event.currentTarget && onClose()}
    >
      <div className="ask__box" role="dialog" aria-modal="true" aria-labelledby="askT">
        <h2 className="ask__t" id="askT">{t('workspace.removeProject.title', { name })}</h2>
        <p className="ask__d">
          {tRich('workspace.removeProject.said', { stays: <b>{t('workspace.removeProject.stays')}</b> })}
        </p>

        <div className="ask__warn" hidden={live === 0}>
          <Icon size="m">
            <path d="M12 9v4M12 17v.01" />
            <path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" />
          </Icon>
          <span>
            {t('workspace.removeProject.live', { count: live })}
          </span>
        </div>

        <button
          className="ask__opt"
          role="checkbox"
          aria-checked={wipe}
          onClick={() => setWipe((was) => !was)}
        >
          <span className="box">
            <Icon size="s">
              <path d="M20 6 9 17l-5-5" />
            </Icon>
          </span>
          <span>
            <span className="ask__ot">{t('workspace.removeProject.wipe')}</span>
            <span className="ask__od">
              {tRich('workspace.removeProject.wipeSaid', { path: <code>{WORKSPACES}</code> })}
            </span>
          </span>
        </button>

        <div className="ask__row">
          <button className="btn" onClick={onClose}>{t('workspace.removeProject.cancel')}</button>
          <button className="btn btn--danger" onClick={() => onConfirm(wipe)}>
            {wipe ? t('workspace.removeProject.removeDelete') : t('workspace.removeProject.remove')}
          </button>
        </div>
      </div>
    </div>
  )
}
