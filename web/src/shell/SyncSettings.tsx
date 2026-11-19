import { useEffect, useState } from 'react'

import type { SyncStatus } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'
import { PrefSwitch } from './PrefSwitch'
import { ago } from './usageFormat'
import { onEvent } from './window'

type Answer = { data: SyncStatus | null; error: string | null }

/*
 * Sync between this person's machines, sealed on each with a key devpit.app
 * never holds. Off unless turned on. A machine joining an account that
 * already syncs takes the key from another machine's offer or from the
 * recovery code; it never makes a second one.
 */
export function SyncSettings(): React.JSX.Element {
  const [sync, setSync] = useState<SyncStatus | null>(null)
  const [busy, setBusy] = useState(false)
  const [said, setSaid] = useState<string | null>(null)
  const [given, setGiven] = useState('')
  const [code, setCode] = useState<string | null>(null)
  const [copied, setCopied] = useState<string | null>(null)

  useEffect(() => {
    const read = (): void => void ask(() => commands.syncRead()).then((answer) => answer.data && setSync(answer.data))
    read()
    return onEvent('sync:changed', read)
  }, [])

  const run = (call: () => Promise<Answer>): void => {
    setBusy(true)
    void call().then((answer) => {
      setBusy(false)
      setSaid(answer.error)
      if (answer.data) setSync(answer.data)
    })
  }
  const copy = (text: string): void => void navigator.clipboard?.writeText(text).then(() => setCopied(text))

  if (!sync) return <></>
  const offer = sync.offering
  return (
    <PrefSwitch
      on={sync.on}
      disabled={busy || (!sync.on && !sync.signedIn)}
      onFlip={() => run(() => ask(() => commands.syncSet(!sync.on)))}
      title={t('workspace.sync.title')}
      said={
        <>
          {t('workspace.sync.said')}
          {!sync.signedIn && ` ${t('workspace.sync.signInFirst')}`}
        </>
      }
    >
      {sync.on && !sync.hasKey && (
        <>
          <span className="pref__d">
            {tRich('workspace.sync.joinHint', { pair: <b>{t('workspace.sync.pair')}</b> })}
          </span>
          <span className="voice__row">
            <input
              className="voice__in"
              aria-label={t('workspace.sync.codeLabel')}
              placeholder={t('workspace.sync.codePlaceholder')}
              autoComplete="off"
              spellCheck={false}
              value={given}
              onChange={(event) => setGiven(event.target.value)}
            />
            <button className="btn" disabled={busy || given.trim() === ''} onClick={() => run(() => ask(() => commands.syncJoin(given)).then((answer) => (answer.data && setGiven(''), answer)))}>
              {t('workspace.sync.join')}
            </button>
          </span>
          <span className="pref__d">
            {t('workspace.sync.lostBoth')}{' '}
            <button className="plusl" disabled={busy} onClick={() => window.confirm(t('workspace.sync.startOverConfirm')) && run(() => ask(() => commands.syncStartOver()))}>
              {t('workspace.sync.startOver')}
            </button>
          </span>
        </>
      )}
      {sync.on && sync.hasKey && (
        <>
          <span className="pref__d">
            {sync.syncedAt ? t('workspace.sync.synced', { ago: ago(sync.syncedAt, Date.now()) }) : t('workspace.sync.notYet')}{' '}
            <button className="plusl" disabled={busy} onClick={() => run(() => ask(() => commands.syncNow()))}>
              {t('workspace.sync.now')}
            </button>
          </span>
          {sync.machines.length > 0 && (
            <ul className="syncm" aria-label={t('workspace.sync.machines')}>
              {sync.machines.map((machine) => (
                <li key={machine.id}>
                  {machine.name}
                  {machine.thisOne ? ` — ${t('workspace.sync.thisComputer')}` : machine.seenAt ? ` — ${t('workspace.sync.seen', { ago: ago(machine.seenAt, Date.now()) })}` : ''}
                </li>
              ))}
            </ul>
          )}
          {offer ? (
            <>
              <span className="pref__d">{t('workspace.sync.offerHint')}</span>
              <span className="voice__row">
                <code className="remote__addr">{offer.text}</code>
                <button className="btn" onClick={() => copy(offer.text)}>
                  {copied === offer.text ? t('workspace.sync.copied') : t('workspace.sync.copy')}
                </button>
                <button className="plusl" disabled={busy} onClick={() => run(() => ask(() => commands.syncOfferWithdraw()))}>
                  {t('workspace.sync.withdraw')}
                </button>
              </span>
            </>
          ) : (
            <span className="voice__row">
              <button className="btn" disabled={busy} onClick={() => run(() => ask(() => commands.syncOffer()))}>
                {t('workspace.sync.pair')}
              </button>
            </span>
          )}
          {code ? (
            <span className="voice__row">
              <code className="remote__addr">{code}</code>
              <button className="btn" onClick={() => copy(code)}>
                {copied === code ? t('workspace.sync.copied') : t('workspace.sync.copy')}
              </button>
              <button className="plusl" onClick={() => setCode(null)}>
                {t('workspace.sync.hide')}
              </button>
            </span>
          ) : (
            <span className="pref__d">
              {t('workspace.sync.recoveryHint')}{' '}
              <button className="plusl" onClick={() => void ask(() => commands.syncRecovery()).then((answer) => answer.data && setCode(answer.data.code))}>
                {t('workspace.sync.showRecovery')}
              </button>
            </span>
          )}
        </>
      )}
      {(said ?? sync.problem) && <span className="pref__d voice__bad">{said ?? sync.problem}</span>}
    </PrefSwitch>
  )
}
