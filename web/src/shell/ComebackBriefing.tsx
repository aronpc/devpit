import { useEffect, useState } from 'react'

import type { Briefing } from '../gen/bindings'
import { t } from '../i18n'
import { onCameBack } from './cameBack'
import { offsetNow } from './channelRules'
import { ask, commands } from './live'
import { zoneNow } from './schedules'

/*
 * What happened while the person was away, above the orchestrator's composer:
 * the sessions, the pull requests merged and the decisions waiting. Only with
 * Assistant mode on — the backend answers nothing otherwise — and only after
 * an absence long enough to have missed something.
 */

/** Away at least this long before coming back is briefed. */
export const AWAY_MS = 30 * 60 * 1000

export function ComebackBriefing({ projectId }: { projectId: string }): React.JSX.Element | null {
  const [briefing, setBriefing] = useState<Briefing | null>(null)

  useEffect(() => {
    let left: number | null = null
    const leave = (): void => {
      left ??= Date.now()
    }
    const hidden = (): void => {
      if (document.visibilityState === 'hidden') leave()
    }
    /* The window's zone, given to the routines on every return: a clock
       change moves them with it, whether or not Schedules is open. */
    const zone = (): void => void ask(() => commands.schedulesList(projectId, zoneNow(), offsetNow()))
    zone()
    window.addEventListener('blur', leave)
    document.addEventListener('visibilitychange', hidden)
    const back = onCameBack(() => {
      zone()
      const since = left
      left = null
      if (since === null || Date.now() - since < AWAY_MS) return
      void ask(() => commands.assistantBriefing(projectId, Math.floor(since / 1000))).then((said) => setBriefing(said.data ?? null))
    })
    return () => {
      window.removeEventListener('blur', leave)
      document.removeEventListener('visibilitychange', hidden)
      back()
    }
  }, [projectId])

  if (!briefing) return null
  return (
    <div className="pref comeback" role="status" aria-label={t('workspace.comeback.title')}>
      <span className="pref__body">
        <span className="pref__t">{t('workspace.comeback.title')}</span>
        {briefing.lines.map((line) => (
          <span key={line} className="pref__d">{line}</span>
        ))}
      </span>
      <button className="btn btn--sm" onClick={() => setBriefing(null)}>{t('workspace.comeback.dismiss')}</button>
    </div>
  )
}
