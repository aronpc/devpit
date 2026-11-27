import { useCallback, useEffect, useState } from 'react'

import type { LiveSession, StopAsked } from '../gen/bindings'
import { ask, commands } from './live'
import { onCarried } from './window'
import { t } from '../i18n'
import { CardState } from './CardState'

/*
 * The stops the orchestrator asked for that the person's own words did not
 * cover, above its composer: every session listed with what it would lose,
 * and one click stops them all, as the person. Nothing stops without it, and
 * one that would lose more by the click than it showed is not stopped.
 */

/** Said by the window's backend when an orchestrator asks for a stop. */
const STOP_ASKED = 'orchestrator:stop-asked'

export function StopsAsked({
  profileId,
  sessions,
  onDone,
}: {
  profileId: string
  sessions: readonly LiveSession[]
  onDone: () => void
}): React.JSX.Element | null {
  const [asked, setAsked] = useState<readonly StopAsked[]>([])
  const [stopping, setStopping] = useState(false)
  const [said, setSaid] = useState<string | null>(null)

  const read = useCallback(() => {
    void commands.orchestratorStopsAsked(profileId).then((answer) => setAsked(answer.stops))
  }, [profileId])
  useEffect(read, [read])
  useEffect(() => onCarried<string>(STOP_ASKED, (id) => id === profileId && read()), [profileId, read])

  /* A session that ended meanwhile has nothing left to stop. */
  const shown = asked.filter((one) => sessions.some((live) => live.name === one.name && (one.pid === null || live.pid === one.pid)))
  if (shown.length === 0 && !said) return null

  /* The click confirms the stops as shown: each stops on the loss it showed. */
  const stopAll = (): void => {
    setStopping(true)
    void ask(() => commands.orchestratorStopsConfirm(profileId, shown)).then((done) => {
      setStopping(false)
      const changed = (done.data?.changed ?? []).map((one) => t('workspace.stopsAsked.changed', { name: one }))
      const said = [done.error, ...(done.data?.failed ?? []), ...changed].filter(Boolean)
      setSaid(said.length > 0 ? said.join(' · ') : null)
      read()
      onDone()
    })
  }

  const keep = (names: readonly string[]): void => {
    setSaid(null)
    void commands.orchestratorStopsDrop(profileId, [...names]).then(read)
  }

  const many = shown.length > 1
  return (
    <section className="wprompt" aria-label={t('workspace.stopsAsked.label')}>
      {shown.length > 0 && (
        <>
          <p className="wprompt__t"><CardState state="waiting" />{many ? t('workspace.stopsAsked.stopMany', { count: shown.length }) : t('workspace.stopsAsked.stopOne')} {t('workspace.stopsAsked.lost')}</p>
          {shown.map((one) => (
            <div className="wprompt__i" key={`${one.name}:${one.pid ?? ''}`}>
              <p className="wprompt__who">
                <span className="osess__name">{one.name}</span>
                <span className="osess__where">{sessions.find((live) => live.name === one.name)?.projectName ?? t('workspace.stopsAsked.outside')}</span>
                {many && (
                  <button className="wprompt__o wprompt__esc" disabled={stopping} aria-label={t('workspace.stopsAsked.keepLabel', { name: one.name })} onClick={() => keep([one.name])}>
                    {t('workspace.stopsAsked.keep')}
                  </button>
                )}
              </p>
              {one.losing && <p className="osess__said">{one.losing}</p>}
            </div>
          ))}
          <div className="wprompt__opts">
            <button className="wprompt__o" disabled={stopping} onClick={stopAll}>
              {many ? t('workspace.stopsAsked.stopAll', { count: shown.length }) : t('workspace.stopsAsked.stopIt')}
            </button>
            <button className="wprompt__o wprompt__esc" disabled={stopping} onClick={() => keep(shown.map((one) => one.name))}>
              {t('workspace.stopsAsked.keepRunning')}
            </button>
          </div>
        </>
      )}
      {said && (
        <p className="osess__said">
          {said}{' '}
          <button className="wprompt__o wprompt__esc" aria-label={t('workspace.stopsAsked.dismiss')} onClick={() => setSaid(null)}>
            ×
          </button>
        </p>
      )}
    </section>
  )
}
