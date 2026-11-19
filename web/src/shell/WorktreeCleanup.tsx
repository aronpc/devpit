import { useEffect, useState } from 'react'

import type { CleanupRun, SweptWhy, WorktreeCleanup as Cleanup } from '../gen/bindings'
import { t } from '../i18n'
import { bytes } from './disk'
import { ask, commands } from './live'
import { PrefRow } from './PrefRow'
import { PrefSwitch } from './PrefSwitch'
import { ago } from './usageFormat'

/*
 * Merged cards' worktrees put away: on its own every hour, or now. What the
 * last run removed, kept and freed is said under the button.
 */

const KEPT: Record<SweptWhy, string> = {
  landed: 'workspace.worktreeCleanup.kept.landed',
  uncommitted: 'workspace.worktreeCleanup.kept.uncommitted',
  running: 'workspace.worktreeCleanup.kept.running',
  failed: 'workspace.worktreeCleanup.kept.failed',
}

function lastRun(run: CleanupRun, nowMs: number): string {
  return t('workspace.worktreeCleanup.lastRun', { ago: ago(run.at, nowMs), removed: run.removed.length, kept: run.kept.length, freed: bytes(run.freedBytes) ?? '0 B' })
}

export function WorktreeCleanup(): React.JSX.Element {
  const [cleanup, setCleanup] = useState<Cleanup | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  /* When the answer came, so "ago" is read from then rather than during render. */
  const [heardAt, setHeardAt] = useState(0)
  const take = (data: Cleanup | null): void => {
    setCleanup(data)
    setHeardAt(Date.now())
  }

  useEffect(() => {
    void ask(() => commands.worktreeCleanupRead()).then((answer) => take(answer.data ?? null))
  }, [])

  const flip = (): void => {
    if (!cleanup) return
    void ask(() => commands.worktreeCleanupWrite(!cleanup.deleteMerged)).then((answer) => answer.data && take(answer.data))
  }

  const now = (): void => {
    setBusy(true)
    void ask(() => commands.worktreeCleanupNow()).then((answer) => {
      setBusy(false)
      setError(answer.error)
      if (answer.data) take(answer.data)
    })
  }

  const last = cleanup?.last ?? null
  return (
    <>
      <PrefSwitch
        on={cleanup?.deleteMerged ?? true}
        onFlip={flip}
        title={t('workspace.worktreeCleanup.title')}
        said={t('workspace.worktreeCleanup.said')}
      />
      <PrefRow title={t('workspace.worktreeCleanup.now')} said={last ? lastRun(last, heardAt) : t('workspace.worktreeCleanup.notRun')}>
        <button className="btn" disabled={busy} onClick={now}>
          {busy ? t('workspace.worktreeCleanup.deleting') : t('workspace.worktreeCleanup.delete')}
        </button>
      </PrefRow>
      {error && <p className="pref__d">{error}</p>}
      {last?.kept.map((row) => (
        <p className="pref__d" key={row.cardId}>
          {t('workspace.worktreeCleanup.kept', { title: row.title, why: t(KEPT[row.why]) })}
          {row.detail ? ` (${row.detail})` : ''}
        </p>
      ))}
    </>
  )
}
