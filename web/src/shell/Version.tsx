import { useEffect, useState } from 'react'

import type { AppInfo } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { SHOW_THE_UPDATE } from './UpdateCard'
import { aNewerOne, useUpdateStatus } from './useUpdateStatus'
import { useCopied } from './useCopied'
import { Icon } from './Icon'

/*
 * Which devpit this is, at the left end of the status strip.
 *
 * Asked of the app, like the Updates row: the number the updater compares
 * against is the number worth showing. A click copies it with the platform —
 * what a bug report starts with — or, when a newer one is on its way, brings
 * the update card forward instead, and a dot says so before anyone clicks.
 */

export function Version(): React.JSX.Element | null {
  const [info, setInfo] = useState<AppInfo | null>(null)
  const [status] = useUpdateStatus()
  const [copied, copyText] = useCopied()

  useEffect(() => {
    void ask(() => commands.appInfo()).then((answer) => setInfo(answer.data))
  }, [])

  if (!info) return null
  const named = `devpit ${info.version}${info.dev ? ' (dev)' : ''} · ${info.platform}`
  const newer = aNewerOne(status)
  const which = status && 'version' in status ? ` ${status.version}` : ''

  const copy = (): void => copyText(named)

  return (
    <button
      className="strip__v"
      title={newer ? t('workspace.version.waiting', { version: which }) : copied ? t('workspace.version.copied') : t('workspace.version.clickToCopy', { name: named })}
      onClick={newer ? () => window.dispatchEvent(new Event(SHOW_THE_UPDATE)) : copy}
    >
      {copied ? t('workspace.version.copied') : `v${info.version}${info.dev ? ' · dev' : ''}`}
      {newer && (
        <Icon size="s" className="strip__up" aria-hidden>
          <path d="M12 19V5M5 12l7-7 7 7" />
        </Icon>
      )}
    </button>
  )
}
