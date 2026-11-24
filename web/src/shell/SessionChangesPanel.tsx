import type { SessionChanges } from '../gen/bindings'
import { DiffFiles } from './DiffFiles'
import { mark } from './tree'
import { t } from '../i18n'

/*
 * What a session has changed, beside its terminal: the files, the commits on
 * top of where it began, and the diff — so whether it is clean, and what it
 * did, is answered without leaving the orchestrator.
 */

export function SessionChangesPanel({
  changes,
  problem,
  onReload,
}: {
  changes: SessionChanges | null
  problem: string | null
  onReload: () => void
}): React.JSX.Element {
  return (
    <aside className="sterm__side" aria-label={t('sessions.changes.title')}>
      <div className="sterm__sidehead">
        <span>{t('sessions.changes.title')}</span>
        <button className="sterm__btn" onClick={onReload} title={t('sessions.changes.reloadTitle')}>
          {t('sessions.changes.reload')}
        </button>
      </div>
      {problem && <p className="sterm__note">{problem}</p>}
      {changes && (
        <div className="sterm__sidebody">
          {changes.changes.length === 0 ? (
            <p className="sterm__note">{t('sessions.changes.clean')}</p>
          ) : (
            <ul className="sterm__files">
              {changes.changes.map((change) => (
                <li key={change.path} data-status={change.status}>
                  <span className="sterm__mark">{mark(change.status) ?? ' '}</span>
                  <span className="sterm__path" title={change.path}>
                    {change.path}
                  </span>
                  <span className="sterm__count">
                    {change.added > 0 && <i data-kind="add">+{change.added}</i>}
                    {change.removed > 0 && <i data-kind="del">−{change.removed}</i>}
                  </span>
                </li>
              ))}
            </ul>
          )}
          {changes.commits.length > 0 && (
            <>
              <h3 className="sterm__h">{t('sessions.changes.commits', { base: changes.base ?? '' })}</h3>
              <ul className="sterm__commits">
                {changes.commits.map((commit) => (
                  <li key={commit.sha}>
                    <code>{commit.sha}</code> {commit.subject}
                  </li>
                ))}
              </ul>
            </>
          )}
          {changes.diff && <DiffFiles diff={changes.diff} />}
          {changes.cut && <p className="sterm__note">{t('sessions.changes.cut')}</p>}
        </div>
      )}
    </aside>
  )
}

/** The branch line in the window's head: where it is, and how far it has drifted. */
export function branchWords(changes: SessionChanges): string {
  const drift = [changes.ahead > 0 ? `↑${changes.ahead}` : null, changes.behind > 0 ? `↓${changes.behind}` : null].filter(Boolean).join(' ')
  const where = changes.worktree ? changes.folder : t('sessions.changes.projectFolder')
  return [changes.branch, drift || null, where].filter(Boolean).join(' · ')
}
