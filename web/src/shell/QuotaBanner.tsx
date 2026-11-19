import { useCallback, useEffect, useState } from 'react'

import type { AccountQuota, Accounts } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { accountOf, elsewhere, withDefault } from './accounts'
import { ask, commands } from './live'
import { useShell } from './useShell'

/* Asked once a minute: the backend keeps a read that long anyway. */
const EVERY_MS = 60_000

/*
 * Said when the account this project's sessions open in is nearly out of its
 * 5-hour window, with one click to open the next session in another account.
 * A session already running keeps the account it started in.
 */
export function QuotaBanner(): React.JSX.Element | null {
  const { project } = useShell()
  const [accounts, setAccounts] = useState<Accounts | null>(null)
  const [quotas, setQuotas] = useState<readonly AccountQuota[]>([])

  const read = useCallback(() => {
    void ask(() => commands.accountsRead()).then((answer) => {
      setAccounts(answer.data)
      if (answer.data && answer.data.accounts.length > 0) {
        void ask(() => commands.accountsQuota()).then((quota) => setQuotas(quota.data?.accounts ?? []))
      }
    })
  }, [])

  useEffect(() => {
    read()
    const timer = window.setInterval(read, EVERY_MS)
    return () => window.clearInterval(timer)
  }, [read])

  if (!project || !accounts) return null
  const current = accountOf(accounts, project.id)
  const out = quotas.find((one) => one.name === current && one.nearlyOut)
  if (!current || !out) return null
  const next = elsewhere(quotas, current)

  const move = (): void => {
    if (!next) return
    void ask(() => commands.accountsSave(withDefault(accounts, project.id, next.name))).then((answer) => {
      if (answer.data) setAccounts(answer.data)
    })
  }

  return (
    <section className="remind" role="alert" aria-label={t('workspace.quota.label')}>
      <div className="remind__list">
        <span className="remind__one">
          <span className="remind__what">
            {tRich('workspace.quota.past', { name: <b>{current}</b> })}
          </span>
          {next && (
            <span className="remind__acts">
              <button className="remind__btn remind__btn--done" onClick={move}>
                {t('workspace.quota.next', { name: next.name })}
              </button>
            </span>
          )}
        </span>
      </div>
    </section>
  )
}
