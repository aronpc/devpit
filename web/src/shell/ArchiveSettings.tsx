import { useEffect, useState } from 'react'

import type { Thread } from '../gen/bindings'
import { t } from '../i18n'
import { formatDate } from '../i18n/format'
import { useLocale } from '../i18n/LocaleRoot'
import { ask, commands } from './live'
import { NO_MARKS } from './useConversations'
import { useShell } from './useShell'

/* Settings → Archive: the project's archived conversations, each with a
   way back. Archiving keeps the transcript; it only leaves the lists. */

export function ArchiveSettings(): React.JSX.Element {
  const { project, reload } = useShell()
  /* Hears the locale, so the rows re-say themselves when it changes. */
  useLocale()
  const [archived, setArchived] = useState<readonly Thread[] | null>(null)
  const projectId = project?.id ?? null

  useEffect(() => {
    if (!projectId) return
    let live = true
    void ask(() => commands.chatArchived(projectId)).then((answer) => {
      if (live) setArchived(answer.data?.conversations ?? [])
    })
    return () => {
      live = false
    }
  }, [projectId])

  const restore = (one: Thread): void => {
    if (!projectId) return
    void ask(() => commands.chatMark(projectId, one.id, { ...NO_MARKS, ...one.marks, archived: false })).then((answer) => {
      if (!answer.data) return
      setArchived((was) => (was ?? []).filter((other) => other.id !== one.id))
      reload()
    })
  }

  return (
    <>
      <p className="acc__note">
        {t('archive.note', { name: project?.name ?? t('archive.thisProject') })}
      </p>
      {(!projectId || archived?.length === 0) && <p className="prefs__none">{t('archive.none')}</p>}
      {projectId && archived?.map((one) => (
        <div className="pref" key={one.id}>
          <span className="pref__body">
            <span className="pref__t">{one.marks?.name ?? one.title}</span>
            <span className="pref__d">{one.lastAt ? formatDate(one.lastAt * 1000) : null}</span>
          </span>
          <span className="prefs__acts">
            <button className="btn" onClick={() => restore(one)}>
              {t('archive.restore')}
            </button>
          </span>
        </div>
      ))}
    </>
  )
}
