import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { formatDate } from '../i18n/format'
import { useLocale } from '../i18n/LocaleRoot'
import { tRich } from '../i18n/rich'
import type { RemoteActivity, RemoteAsking, RemoteDevice, RemotePairing, RemoteVia, RemoteView } from '../gen/bindings'
import { ask, commands } from './live'
import { PrefSwitch } from './PrefSwitch'
import { RELAY_DOC, RemoteMcp } from './RemoteMcp'
import { launcherLink } from './remoteLauncher'
import { onCarried } from './window'
import { useCopied } from './useCopied'

/*
 * Remote: this machine, from the person's other devices — over their
 * tailnet, or from anywhere through devpit.app's relay, encrypted end to end.
 * Off unless turned on; every device paired here, from this screen; what each
 * may do besides watching granted here, and only here.
 */

/* Paths the sentences point at: said the same in every language. */
const REMOTE_LOG = '~/.devpit/remote-log.jsonl'

export function RemoteSettings(): React.JSX.Element {
  useLocale()
  const [view, setView] = useState<RemoteView | null>(null)
  const [pairing, setPairing] = useState<RemotePairing | null>(null)
  const [busy, setBusy] = useState(false)
  const [said, setSaid] = useState<string | null>(null)

  const read = (): void => void ask(() => commands.remoteRead()).then((answer) => answer.data && setView(answer.data))
  useEffect(read, [])
  useEffect(() => onCarried<null>('remote:changed', () => (read(), setPairing(null))), [])
  useEffect(() => onCarried<number>('remote:viewers', read), [])

  const flip = (): void => {
    if (!view) return
    setBusy(true)
    void ask(() => commands.remoteSet(!view.enabled)).then((answer) => {
      setBusy(false)
      setSaid(answer.error)
      if (answer.data) setView(answer.data)
      if (!answer.data?.enabled) setPairing(null)
    })
  }

  const choose = (via: RemoteVia): void => {
    setBusy(true)
    setPairing(null)
    void ask(() => commands.remoteVia(via)).then((answer) => {
      setBusy(false)
      setSaid(answer.error ?? answer.data?.problem ?? null)
      if (answer.data) setView(answer.data)
    })
  }

  const pair = (): void =>
    void ask(() => commands.remotePair(false)).then((answer) => {
      setSaid(answer.error)
      setPairing(answer.data)
    })

  if (!view) return <></>
  const relay = view.via === 'relay'
  const reachable = relay ? view.relay.online : Boolean(view.address)
  const devices = view.devices.filter((device) => !device.mcp)

  return (
    <>
      <PrefSwitch
        on={view.enabled}
        disabled={busy}
        onFlip={flip}
        title={t('remote.title')}
        said={t('remote.said', { risk: t('remote.risk') })}
        moreLabel={t('remote.howTo')}
        more={relay ? <RelaySteps /> : <TailnetSteps login={view.tailscale.login} />}
      >
        {view.enabled && (
          <>
            <span className="remote__chips" role="group" aria-label={t('remote.reachGroup')}>
              <button className="remote__chip" aria-pressed={!relay} disabled={busy} onClick={() => choose('tailscale')} title={t('remote.tailscaleTip')}>
                {t('remote.tailscale')}
              </button>
              <button className="remote__chip" aria-pressed={relay} disabled={busy} onClick={() => choose('relay')} title={t('remote.relayTip')}>
                {t('remote.relay')}
              </button>
            </span>
            {relay ? <RelayState view={view} /> : <TailnetState view={view} />}
            {said && <span className="pref__d voice__bad">{said}</span>}
            {reachable && !pairing && (
              <span className="voice__row">
                <button className="btn" onClick={pair}>
                  {t('remote.pairDevice')}
                </button>
              </span>
            )}
            {pairing && <PairingCode pairing={pairing} relay={relay} onDone={() => (commands.remotePairCancel(), setPairing(null))} />}
            {view.asking.map((one) => (
              <AskingRow key={one.id} asking={one} onView={setView} />
            ))}
            {devices.map((device) => (
              <DeviceRow key={device.id} device={device} onView={setView} />
            ))}
            {view.devices.length > 0 && <Activity />}
          </>
        )}
      </PrefSwitch>
      {view.enabled && relay && <RemoteMcp view={view} onView={setView} />}
    </>
  )
}

function TailnetSteps({ login }: { login: string | null }): React.JSX.Element {
  return (
    <>
      <ol className="prefhow">
        <li>{t('remote.tailnetStep1', { login: login ? ` (${login})` : '' })}</li>
        <li>{t('remote.tailnetStep2')}</li>
        <li>{tRich('remote.tailnetStep3', { pair: <b>{t('remote.pairDevice')}</b> })}</li>
        <li>{t('remote.tailnetStep4')}</li>
      </ol>
      <p>
        {tRich('remote.tailnetNote', { watched: <i>{t('remote.watchedByOne')}</i>, log: <code>{REMOTE_LOG}</code> })}
      </p>
    </>
  )
}

function RelaySteps(): React.JSX.Element {
  return (
    <>
      <ol className="prefhow">
        <li>{t('remote.relayStep1')}</li>
        <li>{tRich('remote.relayStep2', { pair: <b>{t('remote.pairDevice')}</b> })}</li>
        <li>{t('remote.relayStep3')}</li>
        <li>{tRich('remote.relayStep4', { forget: <b>{t('remote.forget')}</b> })}</li>
      </ol>
      <p>{tRich('remote.relayNote', { doc: <code>{RELAY_DOC}</code> })}</p>
    </>
  )
}

function TailnetState({ view }: { view: RemoteView }): React.JSX.Element {
  const tailscale = view.tailscale
  const missing = !tailscale.installed
    ? t('remote.installTailscale')
    : !tailscale.running
      ? t('remote.loginTailscale')
      : !tailscale.https
        ? t('remote.noHttps')
        : null
  return (
    <>
      {view.address ? <RemoteAddress address={view.address} login={tailscale.login} /> : <span className="pref__d">{view.problem ?? t('remote.notReachable')}</span>}
      {missing && <span className="pref__d">{missing}</span>}
    </>
  )
}

function RelayState({ view }: { view: RemoteView }): React.JSX.Element {
  if (!view.relay.signedIn) return <span className="pref__d">{t('remote.signInFirst')}</span>
  if (view.relay.online) return <span className="pref__d">{t('remote.connected')}</span>
  return <span className="pref__d">{view.relay.problem ?? view.problem ?? t('remote.connecting')}</span>
}

/* The code to scan. Through the relay it is the whole pairing: it carries this
   machine's key and a secret too long to type, so there is no code to read. */
function PairingCode({ pairing, relay, onDone }: { pairing: RemotePairing; relay: boolean; onDone: () => void }): React.JSX.Element {
  const [copied, copyText] = useCopied()
  const copy = (): void => copyText(pairing.url)
  return (
    <span className="remote__pair">
      <span className="remote__qr" dangerouslySetInnerHTML={{ __html: pairing.qrSvg }} />
      <span className="remote__code">
        {relay ? (
          <>
            <span className="pref__d">{t('remote.scanRelay')}</span>
            <button className="btn" onClick={copy}>
              {copied ? t('remote.copied') : t('remote.copyLink')}
            </button>
          </>
        ) : (
          <>
            <span className="pref__d">{t('remote.scanTailnet')}</span>
            <code className="remote__big">{pairing.code}</code>
          </>
        )}
        <span className="pref__d">{t('remote.goodOnce')}</span>
        <button className="btn" onClick={onDone}>
          {t('remote.done')}
        </button>
      </span>
    </span>
  )
}

/* The address to type on the other device, one click from the clipboard:
   copied by hand off a settings pane, a tailnet name is easy to get wrong. */
function RemoteAddress({ address, login }: { address: string; login: string | null }): React.JSX.Element {
  const [copied, copyText] = useCopied()
  const copy = (): void => copyText(address)
  return (
    <>
      <span className="voice__row">
        <code className="remote__addr">{address}</code>
        <button className="btn" onClick={copy}>
          {copied ? t('remote.copied') : t('remote.copy')}
        </button>
      </span>
      <span className="pref__d">{t('remote.openOnTailnet', { login: login ? t('remote.signedInAs', { login }) : '' })}</span>
      <LauncherRow address={address} />
    </>
  )
}

/* What paired devices did lately, from the Remote's log — never what they saw. */
function Activity(): React.JSX.Element {
  const [entries, setEntries] = useState<readonly RemoteActivity[] | null>(null)
  const show = (): void => void ask(() => commands.remoteActivity()).then((answer) => setEntries(answer.data?.entries ?? []))
  if (!entries)
    return (
      <span className="voice__row">
        <button className="btn" onClick={show}>
          {t('remote.showActivity')}
        </button>
      </span>
    )
  return (
    <span className="remote__log">
      {entries.length === 0 && <span className="pref__d">{t('remote.noActivity')}</span>}
      {entries.map((one, at) => (
        <span className="pref__d" key={at}>
          {formatDate((one.at ?? 0) * 1000, { dateStyle: 'short', timeStyle: 'short' })} · {one.device} · {one.what}
        </span>
      ))}
      <button className="btn" onClick={() => setEntries(null)}>
        {t('remote.hideActivity')}
      </button>
    </span>
  )
}

/* devpit.app/remote opens this machine's own address; it is handed only that,
   in the fragment, and needs the tailnet's HTTPS one. */
function LauncherRow({ address }: { address: string }): React.JSX.Element {
  const link = launcherLink(address)
  if (!link) return <span className="pref__d">{t('remote.needsHttps')}</span>
  return (
    <span className="voice__row">
      <button className="btn" onClick={() => void ask(() => commands.urlOpen(link))} title={t('remote.addToAppTip')}>
        {t('remote.addToApp')}
      </button>
    </span>
  )
}

/* One paired device and what it may do. Watching is not a choice, so it is
   drawn as a fact beside the two that are. */
/* A device that proved the pairing code waits for a yes here: a code seen
   over a shoulder does not pair one by itself. */
function AskingRow({ asking, onView }: { asking: RemoteAsking; onView: (view: RemoteView) => void }): React.JSX.Element {
  const answer = (allow: boolean): void => void ask(() => commands.remoteAnswer(asking.id, allow)).then((answer) => answer.data && onView(answer.data))
  return (
    <span className="remote__dev" role="alert">
      <span className="remote__name">{t('remote.asks', { name: asking.name })}</span>
      <button className="btn" onClick={() => answer(true)}>
        {t('remote.allow')}
      </button>
      <button className="btn" onClick={() => answer(false)}>
        {t('remote.refuse')}
      </button>
    </span>
  )
}

function DeviceRow({ device, onView }: { device: RemoteDevice; onView: (view: RemoteView) => void }): React.JSX.Element {
  const allow = (typing: boolean, answering: boolean): void =>
    void ask(() => commands.remoteAllow(device.id, typing, answering)).then((answer) => answer.data && onView(answer.data))
  return (
    <span className="remote__dev">
      <span className="remote__name" title={t(device.relay ? 'remote.pairedRelay' : 'remote.pairedTailnet')}>
        <i className="remote__dot" data-on={device.connected ? 'true' : undefined} />
        {device.name}
      </span>
      <span className="remote__chips" role="group" aria-label={t('remote.mayDo', { name: device.name })}>
        <span className="remote__chip" data-on="true" title={t('remote.watchTip')}>
          {t('remote.watch')}
        </span>
        <button className="remote__chip" aria-pressed={device.typing} title={t('remote.typeTip')} onClick={() => allow(!device.typing, device.answering)}>
          {t('remote.type')}
        </button>
        <button className="remote__chip" aria-pressed={device.answering} title={t('remote.answerTip')} onClick={() => allow(device.typing, !device.answering)}>
          {t('remote.answer')}
        </button>
      </span>
      <button className="btn" onClick={() => void ask(() => commands.remoteForget(device.id)).then((answer) => answer.data && onView(answer.data))}>
        {t('remote.forget')}
      </button>
    </span>
  )
}
