import { useEffect, useState } from 'react'

import type { SessionRelay } from '../gen/bindings'
import { ask, commands } from './live'
import { PrefSwitch } from './PrefSwitch'
import { t } from '../i18n'

/* The share a handoff starts at when it is turned on. */
const FIRST_SHARE = 70

/*
 * Whether devpit starts sessions on its own: the next card of an epic, and a
 * fresh session when one fills its context. Both off unless turned on here.
 */
export function SessionRelaySettings(): React.JSX.Element {
  const [relay, setRelay] = useState<SessionRelay | null>(null)
  useEffect(() => {
    void ask(() => commands.sessionRelayRead()).then((answer) => setRelay(answer.data ?? null))
  }, [])
  const save = (next: SessionRelay): void => {
    setRelay(next)
    void ask(() => commands.sessionRelayWrite(next)).then((answer) => answer.data && setRelay(answer.data))
  }
  const now = relay ?? { nextCard: false, handoffAtPercent: null }
  const handing = now.handoffAtPercent !== null
  return (
    <>
      <PrefSwitch
        on={now.nextCard}
        onFlip={() => save({ ...now, nextCard: !now.nextCard })}
        title={t('sessions.relay.nextCard')}
        said={t('sessions.relay.nextCardSaid')}
      />
      <PrefSwitch
        on={handing}
        onFlip={() => save({ ...now, handoffAtPercent: handing ? null : FIRST_SHARE })}
        title={t('sessions.relay.handoff')}
        said={t('sessions.relay.handoffSaid')}
      >
        {handing && (
          <label className="voice__row">
            <span className="voice__l">{t('sessions.relay.at')}</span>
            <input
              className="voice__in voice__in--short"
              type="number"
              min="1"
              max="100"
              aria-label={t('sessions.relay.atLabel')}
              defaultValue={now.handoffAtPercent ?? FIRST_SHARE}
              onBlur={(event) => {
                const share = Math.round(Number(event.target.value))
                if (share >= 1 && share <= 100 && share !== now.handoffAtPercent) save({ ...now, handoffAtPercent: share })
              }}
            />
            <span className="voice__l">{t('sessions.relay.ofContext')}</span>
          </label>
        )}
      </PrefSwitch>
    </>
  )
}
