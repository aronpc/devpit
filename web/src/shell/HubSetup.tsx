import { useEffect, useState } from 'react'

import type { HubStatus } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'

/*
 * devpit.app as a channel: it e-mails what waits on you and wakes your phone,
 * set up at devpit.app/account. What leaves this machine is only that
 * something waits, unless titles are on; a button pressed there is done here.
 */
export function HubSetup({ onChanged }: { onChanged: () => void }): React.JSX.Element {
  const [status, setStatus] = useState<HubStatus | null>(null)
  useEffect(() => {
    void ask(() => commands.hubStatus()).then((answer) => setStatus(answer.data))
  }, [])
  const set = (on: boolean, titles: boolean): void =>
    void ask(() => commands.hubSet(on, titles)).then((answer) => {
      if (answer.data) setStatus(answer.data)
      onChanged()
    })
  if (!status) return <></>
  return (
    <div className="pref">
      <span className="pref__body">
        <span className="pref__t">{t('workspace.hub.title')}</span>
        <span className="pref__d">
          {tRich('workspace.hub.said', {
            account: (
              <button className="plusl" onClick={() => void ask(() => commands.urlOpen('https://devpit.app/account'))}>
                devpit.app/account{/* // ui-text: ok an address */}
              </button>
            ),
          })}
          {!status.signedIn && ` ${t('workspace.hub.signIn')}`}
        </span>
        <label className="voice__row">
          <input type="checkbox" disabled={!status.signedIn} checked={status.on} onChange={(event) => set(event.target.checked, status.titles)} />
          {t('workspace.hub.send')}
        </label>
        {status.on && (
          <label className="voice__row">
            <input type="checkbox" checked={status.titles} onChange={(event) => set(status.on, event.target.checked)} />
            {t('workspace.hub.titles')}
          </label>
        )}
      </span>
    </div>
  )
}
