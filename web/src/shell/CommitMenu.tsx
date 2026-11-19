import { useEffect, useState } from 'react'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'

import type { Commit, CommitRef } from '../gen/bindings'
import { ask, commands } from './live'
import { Menu, MenuItem, MenuRule } from './Menu'
import { went } from './problems'
import { t } from '../i18n'

/*
 * Right-click on a commit in History: open it, copy what identifies it, and
 * go to its page on the remote.
 *
 * The full id and the page are asked for as the menu opens — the row only
 * carries the abbreviated id — so "Open on …" is offered only where the
 * checkout has a remote with a web page.
 */
export interface CommitAt {
  readonly commit: Commit
  readonly x: number
  readonly y: number
}

export function CommitMenu({
  projectId,
  at,
  onOpen,
  onClose,
}: {
  projectId: string
  at: CommitAt
  onOpen: (commit: Commit) => void
  onClose: () => void
}): React.JSX.Element {
  const [link, setLink] = useState<CommitRef | null>(null)

  useEffect(() => {
    let live = true
    void ask(() => commands.projectCommitLink(projectId, null, at.commit.sha)).then((answer) => live && setLink(answer.data ?? null))
    return () => {
      live = false
    }
  }, [projectId, at.commit.sha])

  const then = (act: () => void) => (): void => {
    onClose()
    act()
  }
  const copy = (text: string): void => void writeText(text).catch(() => went({ error: t('workspace.commitMenu.copyFailed') }))
  const host = link?.url ? new URL(link.url).hostname.replace(/^www\./, '') : null

  return (
    <Menu at={at} label={t('workspace.commitMenu.label')} onClose={onClose}>
      <MenuItem label={t('workspace.commitMenu.open')} onPick={then(() => onOpen(at.commit))} />
      {link?.url && <MenuItem label={t('workspace.commitMenu.openOn', { host: host ?? '' })} onPick={then(() => void ask(() => commands.urlOpen(link.url ?? '')).then(went))} />}
      <MenuRule />
      <MenuItem label={t('workspace.commitMenu.copyHash')} onPick={then(() => copy(link?.full ?? at.commit.sha))} />
      <MenuItem label={t('workspace.commitMenu.copyShort')} onPick={then(() => copy(at.commit.sha))} />
      <MenuItem label={t('workspace.commitMenu.copyMessage')} onPick={then(() => copy(at.commit.subject))} />
      {link?.url && <MenuItem label={t('workspace.commitMenu.copyLink')} onPick={then(() => copy(link.url ?? ''))} />}
    </Menu>
  )
}
