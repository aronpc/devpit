import type { Part } from '../gen/bindings'
import { byAgent, totals, type TurnFile } from './turnChanges'
import { useShellPick } from './shellStore'
import { t } from '../i18n'

/*
 * The files a finished turn changed, under the turn that changed them.
 *
 * Measured from the checkout before and after, so it is the whole list and
 * not just the edits the agent reported. A file no edit call names is marked:
 * something the agent ran changed it.
 */

export function TurnChanges({ files, parts }: { files: readonly TurnFile[]; parts: readonly Part[] }): React.JSX.Element | null {
  const show = useShellPick((shell) => shell.show)
  if (files.length === 0) return null
  const { added, removed } = totals(files)

  return (
    <div className="tchg">
      <div className="tchg__h">
        <span>
          {t('sessions.turn.filesChanged', { count: files.length })}
        </span>
        <span className="ecard__add">+{added}</span>
        <span className="ecard__del">−{removed}</span>
      </div>
      {files.map((file) => (
        <button
          key={file.path}
          className="tchg__f"
          onClick={() => show('diff', { id: `diff:${file.path}`, path: file.path, title: t('sessions.turn.diffTitle', { name: file.path.split('/').pop() ?? file.path }) })}
        >
          <span className="tchg__p">{file.path}</span>
          {!byAgent(file, parts) && (
            <span className="tchg__by" title={t('sessions.turn.byCommandTitle')}>
              {t('sessions.turn.byCommand')}
            </span>
          )}
          <span className="ecard__add">+{file.added}</span>
          <span className="ecard__del">−{file.removed}</span>
        </button>
      ))}
    </div>
  )
}
