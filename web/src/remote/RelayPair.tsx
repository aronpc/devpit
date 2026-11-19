import { useEffect, useState } from 'react'

import { deviceName } from './deviceName'
import { holdOffer, type RelayMachine } from './relay'
import { t } from '../i18n'

/*
 * Pairing through devpit.app: the machine's QR code opened this page with
 * its key and a one-time secret. The relay needs this device signed in to
 * the account the machine is signed in to; the machine needs the secret.
 */

const guessName = (): string => deviceName(navigator.userAgent, navigator.maxTouchPoints)

/** Whether this browser is signed in to devpit.app; null while asking. */
function useSignedIn(): boolean | null {
  const [signedIn, setSignedIn] = useState<boolean | null>(null)
  useEffect(() => {
    void fetch('/v1/auth/session', { credentials: 'same-origin' })
      .then((answer) => answer.json() as Promise<{ account?: unknown }>)
      .then((said) => setSignedIn(Boolean(said.account)))
      // Not answered: the relay says it when it is so.
      .catch(() => setSignedIn(true))
  }, [])
  return signedIn
}

export function RelayPair({ machine, onPair }: { machine: RelayMachine; onPair: (name: string) => void }): React.JSX.Element {
  const [name, setName] = useState(guessName)
  const signedIn = useSignedIn()

  if (signedIn === false)
    return (
      <main className="rm rm--center">
        <h1 className="rm__title">{t('remote.relay.signInTitle')}</h1>
        <p className="rm__said">{t('remote.relay.signInSaid', { machine: machine.name })}</p>
        <a className="rm__btn" href="/login?next=/remote" onClick={() => holdOffer(location.hash)}>
          {t('remote.relay.signInButton')}
        </a>
      </main>
    )

  return (
    <main className="rm rm--center">
      <h1 className="rm__title">{t('remote.relay.pairTitle', { machine: machine.name })}</h1>
      <p className="rm__said">{t('remote.relay.pairSaid', { machine: machine.name })}</p>
      <label className="rm__field">
        <span>{t('remote.relay.thisDevice')}</span>
        <input value={name} onChange={(event) => setName(event.target.value)} />
      </label>
      <button className="rm__btn" disabled={signedIn === null} onClick={() => onPair(name.trim() || guessName())}>
        {t('remote.relay.pair')}
      </button>
    </main>
  )
}

/* devpit.app's viewer with nothing paired yet: the way in is the machine's code. */
export function RelayNothing(): React.JSX.Element {
  return (
    <main className="rm rm--center">
      <h1 className="rm__title">{t('remote.relay.nothingTitle')}</h1>
      <p className="rm__said">{t('remote.relay.nothingSaid')}</p>
    </main>
  )
}
