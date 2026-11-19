import { useEffect, useState } from 'react'

import type { PlusStatus } from '../gen/bindings'
import { ask, commands } from './live'
import { t } from '../i18n'
import { formatDate } from '../i18n/format'

/* devpit Plus on this install, once this build checks it at all. Until then
   nothing is drawn: there is no Plus to show. */
export function PlusLine(): React.JSX.Element | null {
  const [status, setStatus] = useState<PlusStatus | null>(null)
  useEffect(() => {
    void ask(() => commands.plusStatus()).then((answer) => setStatus(answer.data))
  }, [])
  if (!status?.enabled) return null
  const until = status.expiresAt ? formatDate(status.expiresAt * 1000, { dateStyle: 'short' }) : null
  return (
    <p className="acc__note">
      {status.active ? t('sessions.plus.active', { plan: status.plan ?? '', until: until ?? '—' }) : t('sessions.plus.inactive')}{' '}
      <button className="plusl" onClick={() => void ask(() => commands.plusRefresh()).then((answer) => answer.data && setStatus(answer.data))}>
        {t('sessions.plus.check')}
      </button>
    </p>
  )
}
