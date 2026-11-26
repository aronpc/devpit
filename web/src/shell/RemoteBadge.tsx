import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { commands } from './live'
import { onCarried } from './window'

/*
 * Said on the desk while another device watches this machine: how many, and
 * a click that drops them all. Remote is never quiet about itself.
 */

export function RemoteBadge(): React.JSX.Element | null {
  const [watching, setWatching] = useState(0)
  useEffect(() => onCarried<number>('remote:viewers', setWatching), [])
  if (watching === 0) return null
  return (
    <button className="strip__remote" onClick={() => commands.remoteDrop()} title={t('workspace.remoteBadge.title')}>
      <svg className="strip__remote-ico" aria-hidden width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12Z" />
        <circle cx="12" cy="12" r="3" />
      </svg>
      {t('workspace.remoteBadge.watched', { count: watching })}
    </button>
  )
}
