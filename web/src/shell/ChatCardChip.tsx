import type { ChatCard } from './useChat'
import { useShell } from './useShell'
import { t } from '../i18n'

/*
 * The card a conversation is about, in the chat's corner.
 *
 * It opens the card while the card is on a board. Archived, the card is only a
 * name: a chip that opened nothing would be a control that lies.
 */
export function ChatCardChip({ card }: { card: ChatCard | null }): React.JSX.Element | null {
  const { show, openCard } = useShell()
  if (!card) return null
  const name = card.title ?? t('sessions.aCard')
  if (!card.onBoard) {
    return (
      <span className="pcorner__card" title={t('sessions.chat.cardGone')}>
        {name}
      </span>
    )
  }
  return (
    <button
      className="pcorner__card"
      title={t('sessions.chat.cardOpen')}
      onClick={() => {
        show('board')
        openCard(card.id)
      }}
    >
      {name}
    </button>
  )
}
