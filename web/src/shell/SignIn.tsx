import { useEffect } from 'react'

import mark from '../assets/brand/mark.png'
import { useShell } from './useShell'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'

/* Signing in.
 *
 * One button, because there is one way in: the browser. devpit never sees a
 * password — it asks the accounts site for a short code, opens the browser on
 * the page that approves it, and waits. Whatever the site grows later (GitHub,
 * Google, a passkey) arrives without this sheet changing at all.
 */
export function SignIn({ onClose }: { onClose: () => void }): React.JSX.Element {
  const { membership } = useShell()
  const { signingIn, failed, origin, account } = membership

  /* The sheet's job ends when there is an account. Closing on the click that
     opened the browser would have closed it before anything happened. */
  useEffect(() => {
    if (account) onClose()
  }, [account, onClose])

  const close = (): void => {
    membership.cancelSignIn()
    onClose()
  }

  return (
    <div
      className="auth"
      data-open="true"
      onClick={(event) => event.target === event.currentTarget && close()}
    >
      <div className="auth__box" role="dialog" aria-label={t('workspace.signIn.dialog')}>
        <button className="auth__x" aria-label={t('workspace.signIn.close')} title={t('workspace.signIn.close')} onClick={close}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
        </button>
        <span className="auth__mark"><img
            className="mark"
            alt="" // ui-text: ok an empty literal — the walk misreads it as the rest of the line
            src={mark}
          /></span>

        {signingIn ? (
          <>
            <h2 className="auth__t">{t('workspace.signIn.approve')}</h2>
            <p className="auth__d">
              {tRich('workspace.signIn.browserOpen', { host: <b>{host(origin)}</b> })}
            </p>
            <div className="auth__code">{signingIn.userCode}</div>
            <p className="auth__fine">{t('workspace.signIn.waiting')}</p>
          </>
        ) : (
          <>
            <h2 className="auth__t">{t('workspace.signIn.title')}</h2>
            <p className="auth__d">
              {t('workspace.signIn.free')}
            </p>
            {failed && (
              <p className="auth__failed" role="alert">
                {failed}
              </p>
            )}
            <button className="auth__b" onClick={() => void membership.signIn()}>
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4M10 17l5-5-5-5M15 12H3" /></svg>
              {t('workspace.signIn.continue')}
            </button>
            <p className="auth__fine">
              {t('workspace.signIn.fine')}
            </p>
          </>
        )}
      </div>
    </div>
  )
}

/* The host on its own: a person checking that the page in front of them is the
   one the app opened does not need the scheme to do it. */
function host(origin: string): string {
  try {
    return new URL(origin).host
  } catch {
    return origin
  }
}
