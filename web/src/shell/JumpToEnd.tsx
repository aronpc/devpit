import { t } from '../i18n'
import { Icon } from './Icon'

/* Offered only while the thread is not following: at the end there is
   nowhere for it to go. */
export function JumpToEnd({ away, onJump }: { away: boolean; onJump: () => void }): React.JSX.Element | null {
  if (!away) return null
  return (
    <button className="jumpend" onClick={onJump} title={t('workspace.jump')} aria-label={t('workspace.jump')}>
      <Icon size="s"><path d="M12 5v14M5 12l7 7 7-7" /></Icon>
    </button>
  )
}
