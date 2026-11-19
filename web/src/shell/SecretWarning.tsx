import { t } from '../i18n'
import { looksSecret } from './secretShape'

/* Said while it can still be taken out: sent, a key stays in the transcript. */
export function SecretWarning({ text }: { text: string }): React.JSX.Element | null {
  if (!looksSecret(text)) return null
  return (
    <p className="composer__warn" role="status">
      {t('workspace.secret.warning')}
    </p>
  )
}
