import { t } from '../i18n'

/* The composer's one button: send, or — while a turn runs — stop it. The
import { t } from '../i18n'
   second Escape press is shown on it, where the eye goes to stop. */
export function SendButton({ sending, showEsc, can, onSend, onStop, onForce }: {
  sending: boolean
  showEsc: boolean
  can: boolean
  onSend: () => void
  onStop: () => void
  /** Present once a stop has gone unheeded for a while. */
  onForce?: () => void
}): React.JSX.Element {
  if (sending)
    return (
      <>
        {onForce && (
          <button className="send__force" onClick={onForce} title={t('workspace.sendButton.forceTitle')}>
            {t('workspace.sendButton.force')}
          </button>
        )}
        <button className="send" onClick={onStop} aria-label={showEsc ? t('workspace.sendButton.escAgain') : t('workspace.sendButton.stop')} title={showEsc ? t('workspace.sendButton.escAgain') : t('workspace.sendButton.stop')}>
          {showEsc ? (
            <span className="send__esc">{t('workspace.sendButton.esc')}</span>
          ) : (
            <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><rect x="5" y="5" width="14" height="14" rx="2" /></svg>
          )}
        </button>
      </>
    )
  return (
    <button className="send" onClick={onSend} disabled={!can} aria-label={t('workspace.sendButton.send')} title={t('workspace.sendButton.send')}>
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 19V5M5 12l7-7 7 7" /></svg>
    </button>
  )
}
