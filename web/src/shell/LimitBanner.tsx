import { useEffect, useRef, useState } from 'react'

import type { Limit } from '../gen/bindings'
import { t } from '../i18n'
import { clock } from './chat'

const TITLES: Readonly<Record<Limit['kind'], string>> = {
  rate_limit: 'workspace.limit.rate',
  outage: 'workspace.limit.outage',
}

/* A turn the provider refused, under the thread: sent again now, or by itself
   once the limit resets when the CLI said when. */
export function LimitBanner({ limit, onRetry }: { limit: Limit; onRetry: () => void }): React.JSX.Element {
  const resets = limit.resetsAt === null ? null : limit.resetsAt * 1000
  const [waiting, setWaiting] = useState(false)
  /* The latest, so a thread drawn again does not restart the wait. */
  const retry = useRef(onRetry)
  useEffect(() => {
    retry.current = onRetry
  })
  useEffect(() => {
    if (!waiting || resets === null) return
    const timer = window.setTimeout(() => retry.current(), Math.max(0, resets - Date.now()))
    return () => window.clearTimeout(timer)
  }, [waiting, resets])

  return (
    <section className="limitb" aria-label={t(TITLES[limit.kind])}>
      <div className="limitb__t">
        <b>{t(TITLES[limit.kind])}</b>
        {resets !== null && <span className="limitb__at">{t('workspace.limit.resetsAt', { time: clock(resets) })}</span>}
      </div>
      <div className="limitb__row">
        {resets !== null &&
          (waiting ? (
            <button className="btn" onClick={() => setWaiting(false)} title={t('workspace.limit.stopTitle')}>
              {t('workspace.limit.resuming', { time: clock(resets) })}
            </button>
          ) : (
            <button className="btn" onClick={() => setWaiting(true)}>
              {t('workspace.limit.resume')}
            </button>
          ))}
        <button className="btn btn--go" onClick={onRetry}>
          {t('workspace.limit.retry')}
        </button>
      </div>
    </section>
  )
}
