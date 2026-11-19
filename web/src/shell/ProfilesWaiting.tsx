import { useCallback, useEffect, useState } from 'react'

import type { ProfileWaiting } from '../gen/bindings'
import { t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'
import { profilesChanged } from './profiles'
import { onEvent } from './window'

/*
 * Profiles another computer made or changed, and what each waits for here.
 *
 * A profile that arrived runs nothing until the person has looked at it and
 * taken it. A secret never travels, so its token is typed on this machine.
 * A change elsewhere to what a profile runs or where its token goes is the
 * person's to take — or to keep this machine's own, which then travels back.
 */
export function ProfilesWaiting(): React.JSX.Element | null {
  useLocale()
  const [waiting, setWaiting] = useState<readonly ProfileWaiting[]>([])
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(() => {
    void ask(() => commands.agentProfilesWaiting()).then((answer) => setWaiting(answer.data?.profiles ?? []))
  }, [])
  useEffect(refresh, [refresh])
  /* A round may have changed what waits while the pane was open. */
  useEffect(() => onEvent('sync:written', refresh), [refresh])

  /* One change at a time, named by the value shown: a yes covers what was seen. */
  const settle = (id: string, take: boolean, change?: { name: string; value: string }): void => {
    void ask(() => commands.agentProfileSettle(id, change?.name ?? null, change?.value ?? null, take)).then((answer) => {
      setError(answer.error)
      if (answer.data) setWaiting(answer.data.profiles)
      if (!answer.error) profilesChanged()
    })
  }

  if (waiting.length === 0 && !error) return null
  return (
    <div className="acc__note" role="status">
      {waiting.map((one) => (
        <div key={one.id}>
          {one.arrived && (
            <p>
              <strong>{one.label}</strong>{t('waiting.arrived')}{' '}
              {one.changed.length === 0 && (
                <button className="btn" onClick={() => settle(one.id, true)}>
                  {t('waiting.takeIt')}
                </button>
              )}
            </p>
          )}
          {one.needs.length > 0 && (
            <p>
              <strong>{one.label}</strong>{t('waiting.needs', { needs: one.needs.join(', ') })}
            </p>
          )}
          {one.changed.map((change) => (
            <p key={change.name}>
              {tRich('waiting.changed', { label: <strong>{one.label}</strong>, value: <code>{change.value}</code> }, { name: change.name.replace(/^env\./, '') })}{' '}
              <button className="btn" onClick={() => settle(one.id, true, change)}>
                {t('waiting.useIt')}
              </button>{' '}
              <button className="btn" onClick={() => settle(one.id, false, change)}>
                {t('waiting.keepThis')}
              </button>
            </p>
          ))}
        </div>
      ))}
      {error && <p role="alert">{error}</p>}
    </div>
  )
}
