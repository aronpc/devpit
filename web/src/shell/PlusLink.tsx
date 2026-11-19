import { ask, commands } from './live'
import { plusLink, type PlusFrom } from './plusAddress'
import { t } from '../i18n'

/* One quiet line where Plus would help: no popup, nothing sent until the click. */
export function PlusLink({ from, said }: { from: PlusFrom; said: string }): React.JSX.Element {
  return (
    <span className="pref__d">
      {said}{' '}
      <button className="plusl" onClick={() => void ask(() => commands.urlOpen(plusLink(from)))} title={t('workspace.plusLink.title')}>
        {t('workspace.plusLink.join')}
      </button>
    </span>
  )
}
