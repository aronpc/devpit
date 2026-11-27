import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { commands } from './live'
import { onCarried } from './window'
import { Icon } from './Icon'

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
      <Icon size="s" className="strip__remote-ico" aria-hidden>
        <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12Z" />
        <circle cx="12" cy="12" r="3" />
      </Icon>
      {t('workspace.remoteBadge.watched', { count: watching })}
    </button>
  )
}
