import { t } from '../i18n'
import { useCopied } from './useCopied'

/* The CLI's id for this conversation, one click from the clipboard: it is what
   `claude --resume` takes, and what a bug report about a session needs. */
export function CopySession({ id }: { id: string }): React.JSX.Element {
  const [copied, copy] = useCopied()
  return (
    <button
      className="sq26"
      aria-label={copied ? t('sessions.chat.copiedId') : t('sessions.chat.copyId')}
      title={id}
      onClick={() => copy(id)}
    >
      {copied ? (
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="m5 13 4 4L19 7" /></svg>
      ) : (
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M4 9h16M4 15h16M10 3 8 21M16 3l-2 18" /></svg>
      )}
    </button>
  )
}
