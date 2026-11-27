import { t } from '../i18n'
import { Icon } from './Icon'

/* What a card in the chat is waiting for, said in its header: an icon and a
   word, so the state reads without its colour. */
export function CardState({ state }: { state: 'waiting' | 'draft' }): React.JSX.Element {
  return (
    <span className="cstate" data-state={state}>
      <Icon size="s">
        {state === 'waiting' ? <path d="M12 7v5l3 2M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0" /> : <path d="M4 20h4L19 9l-4-4L4 16Zm11-15 4 4" />}
      </Icon>
      {state === 'waiting' ? t('workspace.waitingPrompts.title') : t('workspace.cardState.draft')}
    </span>
  )
}
