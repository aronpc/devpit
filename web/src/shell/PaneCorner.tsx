import { t } from '../i18n'
import { HostTerminals } from './HostTerminals'
import { useShell } from './useShell'
import { useKeys } from './useShortcut'

/*
 * What a pane can do, as a few icons in its corner rather than a title bar.
 *
 * The tab already names the pane, so a bar repeating "Chat" above it was a
 * row of height spent on a word. The corner keeps the actions — a chat, a new
 * terminal, splitting, closing — and whatever the pane adds of its own.
 */

type Direction = 'horizontal' | 'vertical'

export function PaneCorner({
  tabId,
  what,
  onSplit,
  onChat,
  onClose,
  closeLabel,
  closeArmed,
  children,
}: {
  tabId: string
  /* Named in the close button, so a screen reader hears which pane goes. */
  what: string
  /* Offered only by panes that can split; a split button that does nothing
     would be a control that lies. */
  onSplit?: (direction: Direction) => void
  /* Goes on with this pane's own conversation in a chat. Without it, the chat
     icon opens a new one. */
  onChat?: () => void
  /* A pane that is one of several closes itself rather than its whole tab. */
  onClose?: () => void
  closeLabel?: string
  /* The close is waiting for a second press before it stops what runs here. */
  closeArmed?: boolean
  children?: React.ReactNode
}): React.JSX.Element {
  const { show, close, open, openCard } = useShell()
  const keys = useKeys()
  const chatLabel = onChat ? t('workspace.corner.continueChat') : t('workspace.corner.newChat')
  /* The way back to the card a terminal was opened for. Only a tab the backend
     filed under a card has one. */
  const cardId = open.find((tab) => tab.id === tabId)?.cardId

  return (
    <div className="pcorner">
      {cardId && (
        <button
          className="pcorner__card"
          title={t('workspace.corner.cardTitle')}
          onClick={() => {
            show('board')
            openCard(cardId)
          }}
        >
          {t('workspace.corner.card')}
        </button>
      )}
      {children}
      <button
        className="sq26"
        aria-label={chatLabel}
        title={onChat ? t('workspace.corner.continueTitle') : chatLabel}
        onClick={onChat ?? (() => show('chat'))}
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M21 11.5a8.4 8.4 0 0 1-9 8.4 9.9 9.9 0 0 1-4.2-.9L3 20.5l1.6-4.4A8.4 8.4 0 0 1 3.6 11.5 8.4 8.4 0 0 1 12 3.1a8.4 8.4 0 0 1 9 8.4Z" /><path d="M12 8.5v6M9 11.5h6" /></svg>
      </button>
      <button className="sq26" aria-label={t('workspace.corner.newTerminal')} title={t('workspace.corner.newTerminal')} onClick={() => show('term')}>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="m7 9 3 3-3 3M13 15h4" /></svg>
      </button>
      <HostTerminals />
      {onSplit && (
        <>
          <button className="sq26" aria-label={t('workspace.corner.splitRight')} title={t('workspace.corner.splitRightKeys', { keys: keys('splitRight') })} onClick={() => onSplit('horizontal')}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M12 4v16" /></svg>
          </button>
          <button className="sq26" aria-label={t('workspace.corner.splitDown')} title={t('workspace.corner.splitDownKeys', { keys: keys('splitDown') })} onClick={() => onSplit('vertical')}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M3 12h18" /></svg>
          </button>
        </>
      )}
      <button
        className="sq26"
        data-armed={closeArmed ? 'true' : undefined}
        aria-label={closeLabel ?? t('workspace.corner.closeWhat', { what })}
        title={closeLabel ?? t('workspace.corner.close')}
        onClick={onClose ?? (() => close(tabId))}
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
      </button>
    </div>
  )
}
