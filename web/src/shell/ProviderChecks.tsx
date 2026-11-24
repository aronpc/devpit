import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
import { ask, commands } from './live'
import { PrefRow } from './PrefRow'
import { CHECK_DEFAULT, CHECK_MINUTES, checksChanged } from './checkEvery'

/* Settings → Providers: how often sign-ins and MCP servers are checked. */
export function ProviderChecks(): React.JSX.Element {
  useLocale()
  const [minutes, setMinutes] = useState<number | null>(null)
  useEffect(() => {
    void ask(() => commands.providersCheckRead()).then((answer) => setMinutes(answer.data?.minutes ?? CHECK_DEFAULT))
  }, [])
  const choose = (next: number): void => {
    void ask(() => commands.providersCheckWrite(next)).then((answer) => {
      if (!answer.data) return
      setMinutes(answer.data.minutes)
      checksChanged(answer.data.minutes)
    })
  }
  return (
    <PrefRow title={t('checks.title')} said={t('checks.said')}>
      <select aria-label={t('checks.every')} value={minutes ?? CHECK_DEFAULT} disabled={minutes === null} onChange={(event) => choose(Number(event.target.value))}>
        {CHECK_MINUTES.map((one) => (
          <option key={one} value={one}>
            {t('checks.option', { count: one })}
          </option>
        ))}
      </select>
    </PrefRow>
  )
}
