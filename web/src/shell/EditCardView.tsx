import { useState } from 'react'

import { counted, diffText, type EditDiff } from './editCard'
import { ofPath } from './languages'
import { Painted } from './Painted'
import { useShell } from './useShell'
import { useCopied } from './useCopied'
import { t } from '../i18n'

/*
 * One edit, as a card: the file, what it gained and lost, and the lines.
 *
 * The path opens the file when it is inside the project. An agent can write
 * anywhere its sandbox allows, and a tab for a path outside the project would
 * be refused by the reader behind it — so that path is shown, not offered.
 */

/* Past this many rows the card opens folded: one edit should not fill the thread. */
const FOLD_ROWS = 24

export function EditCard({ diff, failed, error }: { diff: EditDiff; failed: boolean; error: string }): React.JSX.Element {
  const { project, show } = useShell()
  const root = project?.rootPath ? `${project.rootPath.replace(/\/$/, '')}/` : null
  const inside = root && diff.path.startsWith(root) ? diff.path.slice(root.length) : null
  const relative = inside ?? (diff.path.startsWith('/') ? null : diff.path)
  const { added, removed } = counted(diff.rows)
  const language = ofPath(diff.path)
  const [whole, setWhole] = useState(false)
  const [copied, copyText] = useCopied()
  const long = diff.rows.length > FOLD_ROWS && !whole
  const rows = long ? diff.rows.slice(0, FOLD_ROWS) : diff.rows

  return (
    <div className="ecard" data-failed={failed || undefined}>
      <div className="ecard__h">
        {relative ? (
          <button className="ecard__p" onClick={() => show('file', { id: `file:${relative}`, path: relative })}>
            {relative}
          </button>
        ) : (
          <span className="ecard__p">{diff.path}</span>
        )}
        {!failed && (
          <span className="ecard__n">
            <span className="ecard__add">+{added}</span> <span className="ecard__del">−{removed}</span>
          </span>
        )}
        {!failed && (
          <button
            className="ecard__c"
            onClick={() => copyText(diffText(diff))}
          >
            {copied ? t('workspace.editCard.copied') : t('workspace.editCard.copy')}
          </button>
        )}
      </div>
      {failed ? (
        <pre className="ecard__err">{error || t('workspace.editCard.failed')}</pre>
      ) : (
        <div className="diff ecard__b">
          {rows.map((row, at) => (
            <div className="diff__l" data-d={row.kind} key={at}>
              {row.text ? <Painted text={row.text} language={language} /> : ' '}
            </div>
          ))}
          {long && (
            <button className="diff__at ecard__more" onClick={() => setWhole(true)}>
              {t('workspace.editCard.showMore', { count: diff.rows.length - FOLD_ROWS })}
            </button>
          )}
          {!long && diff.hidden > 0 && <div className="diff__at">{t('workspace.editCard.hidden', { count: diff.hidden })}</div>}
        </div>
      )}
    </div>
  )
}
