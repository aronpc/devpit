import { useState } from 'react'

import { deviceName } from './deviceName'
import { pair } from './socket'
import { t } from '../i18n'

/*
 * Pairing: the code shown on the machine's screen, typed or carried in the
 * link the QR code opens. Good once, for ten minutes.
 */

const fromLink = (): string => new URLSearchParams(location.hash.slice(1)).get('pair') ?? ''

const guessName = (): string => deviceName(navigator.userAgent, navigator.maxTouchPoints)

export function RemotePair({ onPaired }: { onPaired: (token: string) => void }): React.JSX.Element {
  const [code, setCode] = useState(fromLink)
  const [name, setName] = useState(guessName)
  const [busy, setBusy] = useState(false)
  const [said, setSaid] = useState<string | null>(null)

  const go = async (): Promise<void> => {
    setBusy(true)
    const answer = await pair(code, name.trim() || guessName())
    setBusy(false)
    if ('token' in answer) {
      history.replaceState(null, '', location.pathname)
      return onPaired(answer.token)
    }
    setSaid(answer.error)
  }

  return (
    <main className="rm rm--center">
      <h1 className="rm__title">{t('remote.pair.title')}</h1>
      <p className="rm__said">{t('remote.pair.said')}</p>
      <label className="rm__field">
        <span>{t('remote.pair.code')}</span>
        <input value={code} onChange={(event) => setCode(event.target.value)} autoCapitalize="characters" autoComplete="off" spellCheck={false} />
      </label>
      <label className="rm__field">
        <span>{t('remote.pair.thisDevice')}</span>
        <input value={name} onChange={(event) => setName(event.target.value)} />
      </label>
      {said && <p className="rm__failed">{said}</p>}
      <button className="rm__btn" disabled={busy || code.trim().length < 8} onClick={() => void go()}>
        {t('remote.pair.pair')}
      </button>
      {busy && <p className="rm__dim">{t('remote.pair.allow')}</p>}
    </main>
  )
}
