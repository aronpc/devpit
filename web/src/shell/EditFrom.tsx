import { t } from '../i18n'

/* Asked before an edit goes back, when files changed since the message: they
import { t } from '../i18n'
   can go back too, or stay as they are now. */
export function EditFrom({ files, onPick, onCancel }: { files: number; onPick: (putBack: boolean) => void; onCancel: () => void }): React.JSX.Element {
  return (
    <section className="efrom" aria-label={t('workspace.editFrom.label')}>
      <p className="efrom__t">
        {t('workspace.editFrom.ask', { count: files })}
      </p>
      <div className="efrom__row">
        <button className="btn" onClick={onCancel}>
          {t('workspace.editFrom.cancel')}
        </button>
        <button className="btn" onClick={() => onPick(false)}>
          {t('workspace.editFrom.keep')}
        </button>
        <button className="btn btn--go" onClick={() => onPick(true)}>
          {t('workspace.editFrom.putBack')}
        </button>
      </div>
    </section>
  )
}
