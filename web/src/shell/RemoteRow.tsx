import { useState } from 'react'

import type { RemoteAct, Worktree } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { changed } from './useTree'
import { Icon } from './Icon'

/*
 * The branch against its remote: fetch, pull, push, and sync, which is the
 * two. Pull only fast-forwards — the app refuses to start a merge it cannot
 * finish, and git's own words say what to do instead.
 */

const SAYS: Record<RemoteAct, string> = { fetch: 'workspace.remoteRow.fetched', pull: 'workspace.remoteRow.pulled', push: 'workspace.remoteRow.pushed', sync: 'workspace.remoteRow.synced' }

/** The one act the row leads with, from how far apart the two sides are. */
export function leading(ahead: number, behind: number): RemoteAct {
  if (ahead > 0 && behind > 0) return 'sync'
  if (behind > 0) return 'pull'
  if (ahead > 0) return 'push'
  return 'sync'
}

export function RemoteRow({
  projectId,
  here,
  onDone,
}: {
  projectId: string
  here: Worktree | undefined
  /** Moved the branch: the projects list holds its ahead and behind. */
  onDone: () => void
}): React.JSX.Element {
  const [busy, setBusy] = useState<RemoteAct | null>(null)
  const [said, setSaid] = useState<string | null>(null)
  const ahead = here?.ahead ?? 0
  const behind = here?.behind ?? 0
  const lead = leading(ahead, behind)

  const run = (act: RemoteAct): void => {
    setBusy(act)
    setSaid(null)
    void ask(() => commands.changesRemote(projectId, null, act))
      .then((answer) => {
        setSaid(answer.error ?? t(SAYS[act]))
        changed()
        onDone()
      })
      .finally(() => setBusy(null))
  }

  const button = (act: RemoteAct, label: string, glyph: React.ReactNode, count?: number): React.JSX.Element => (
    <button className="dbtn gremote__b" data-lead={act === lead ? 'true' : undefined} disabled={busy !== null} onClick={() => run(act)} title={label} aria-label={label}>
      {busy === act ? <span className="gremote__spin" aria-hidden="true" /> : glyph}
      {count !== undefined && count > 0 && <span className="gremote__n">{count}</span>}
    </button>
  )

  return (
    <div className="gremote">
      {button('fetch', t('workspace.remoteRow.fetch'), <Arc />)}
      {button('pull', behind > 0 ? t('workspace.remoteRow.pullCommits', { count: behind }) : t('workspace.remoteRow.pull'), <Arrow down />, behind)}
      {button('push', ahead > 0 ? t('workspace.remoteRow.pushCommits', { count: ahead }) : t('workspace.remoteRow.push'), <Arrow />, ahead)}
      <button className="btn gremote__sync" data-lead={lead === 'sync' && ahead + behind > 0 ? 'true' : undefined} disabled={busy !== null} onClick={() => run('sync')} title={t('workspace.remoteRow.syncTitle')}>
        {busy === 'sync' ? t('workspace.remoteRow.syncing') : t('workspace.remoteRow.sync')}
      </button>
      {said && <span className="gremote__said" title={said}>{said}</span>}
    </div>
  )
}

function Arrow({ down }: { down?: boolean }): React.JSX.Element {
  return (
    <Icon size="s" aria-hidden="true">
      <path d={down ? 'M12 5v14M6 13l6 6 6-6' : 'M12 19V5M6 11l6-6 6 6'} />
    </Icon>
  )
}

function Arc(): React.JSX.Element {
  return (
    <Icon size="s" aria-hidden="true">
      <path d="M21 12a9 9 0 1 1-3-6.7L21 8M21 3v5h-5" />
    </Icon>
  )
}
