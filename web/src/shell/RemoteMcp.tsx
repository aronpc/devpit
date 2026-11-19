import { useState } from 'react'

import { t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
import { tRich } from '../i18n/rich'
import type { RemoteDevice, RemotePairing, RemoteView } from '../gen/bindings'
import { ask, commands } from './live'
import { PrefSwitch } from './PrefSwitch'
import { useCopied } from './useCopied'

/*
 * devpit's MCP, read by an MCP client on another computer through
 * devpit.app's relay and encrypted end to end: the board, the sessions and
 * what waits on you, never a change. Off unless turned on here; turning it
 * off, or forgetting a client, cuts it off at once.
 */

/* The protocol's page, which both Remote panes point at. */
export const RELAY_DOC = 'docs/remote-relay.md'
const MCP_COMMAND = 'devpit mcp --remote'
const MCP_EXAMPLE = 'claude mcp add devpit-remote -- devpit mcp --remote'

export function RemoteMcp({ view, onView }: { view: RemoteView; onView: (view: RemoteView) => void }): React.JSX.Element {
  useLocale()
  const [busy, setBusy] = useState(false)
  const [pairing, setPairing] = useState<RemotePairing | null>(null)
  const [said, setSaid] = useState<string | null>(null)
  const clients = view.devices.filter((device) => device.mcp)

  const flip = (): void => {
    setBusy(true)
    setPairing(null)
    void ask(() => commands.remoteMcpSet(!view.mcp)).then((answer) => {
      setBusy(false)
      setSaid(answer.error)
      if (answer.data) onView(answer.data)
    })
  }

  const pair = (): void =>
    void ask(() => commands.remotePair(true)).then((answer) => {
      setSaid(answer.error)
      setPairing(answer.data)
    })

  return (
    <PrefSwitch
      on={view.mcp}
      disabled={busy}
      onFlip={flip}
      title={t('remoteMcp.title')}
      said={t('remoteMcp.said')}
      moreLabel={t('remote.howTo')}
      more={<McpSteps />}
    >
      <span className="pref__d">{t('remoteMcp.limit')}</span>
      {said && <span className="pref__d voice__bad">{said}</span>}
      {view.mcp && view.relay.online && !pairing && (
        <span className="voice__row">
          <button className="btn" onClick={pair}>
            {t('remoteMcp.pair')}
          </button>
        </span>
      )}
      {pairing && <McpPairing pairing={pairing} onDone={() => (commands.remotePairCancel(), setPairing(null))} />}
      {clients.map((client) => (
        <ClientRow key={client.id} client={client} onView={onView} />
      ))}
    </PrefSwitch>
  )
}

function McpSteps(): React.JSX.Element {
  return (
    <>
      <ol className="prefhow">
        <li>{t('remoteMcp.step1')}</li>
        <li>{tRich('remoteMcp.step2', { pair: <b>{t('remoteMcp.pair')}</b> })}</li>
        <li>
          {tRich('remoteMcp.step3', {
            command: <code>{MCP_COMMAND}</code>,
            example: <code>{MCP_EXAMPLE}</code>,
          })}
        </li>
        <li>{tRich('remoteMcp.step4', { forget: <b>{t('remote.forget')}</b> })}</li>
      </ol>
      <p>{tRich('remoteMcp.note', { doc: <code>{RELAY_DOC}</code> })}</p>
    </>
  )
}

/* The command to run on the other computer, with this computer's pairing
   link in it. There is no camera to point there, so no code to scan. */
function McpPairing({ pairing, onDone }: { pairing: RemotePairing; onDone: () => void }): React.JSX.Element {
  const [copied, copyText] = useCopied()
  const command = `devpit mcp --remote pair '${pairing.url}'`
  const copy = (): void => copyText(command)
  return (
    <span className="remote__code">
      <span className="pref__d">{t('remoteMcp.runThis')}</span>
      <code className="remote__addr">{command}</code>
      <span className="voice__row">
        <button className="btn" onClick={copy}>
          {copied ? t('remote.copied') : t('remoteMcp.copyCommand')}
        </button>
        <button className="btn" onClick={onDone}>
          {t('remote.done')}
        </button>
      </span>
      <span className="pref__d">{t('remoteMcp.goodOnce')}</span>
    </span>
  )
}

function ClientRow({ client, onView }: { client: RemoteDevice; onView: (view: RemoteView) => void }): React.JSX.Element {
  return (
    <span className="remote__dev">
      <span className="remote__name" title={t('remoteMcp.clientTip')}>
        <i className="remote__dot" data-on={client.connected ? 'true' : undefined} />
        {client.name}
      </span>
      <span className="remote__chip" data-on="true" title={t('remoteMcp.readOnlyTip')}>
        {t('remoteMcp.readOnly')}
      </span>
      <button className="btn" onClick={() => void ask(() => commands.remoteForget(client.id)).then((answer) => answer.data && onView(answer.data))}>
        {t('remote.forget')}
      </button>
    </span>
  )
}
