import { useEffect, useState } from 'react'

import type { Installation, OutsideSession, Profile } from '../gen/bindings'
import { ask, commands } from './live'
import { profileFor, titled } from './outside'
import { short } from './strip'
import { named } from './useInstallations'
import { useShell } from './useShell'
import { t } from '../i18n'
import { formatDate } from '../i18n/format'

/*
 * Sessions of this project started in a terminal, which devpit never held.
 *
 * Under "Earlier" and apart from it: those are conversations devpit titled
 * with the person's own words, and these carry only the title the CLI wrote.
 * Opening one starts a conversation that resumes it.
 */

export function OutsideThreads(): React.JSX.Element | null {
  const { project, show } = useShell()
  const [sessions, setSessions] = useState<readonly OutsideSession[]>([])
  const [installations, setInstallations] = useState<readonly Installation[]>([])
  const [profiles, setProfiles] = useState<readonly Profile[]>([])
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!project) return
    void Promise.all([
      ask(() => commands.chatOutside(project.id)),
      ask(() => commands.cliInstallations()),
      ask(() => commands.agentProfiles()),
    ]).then(([found, installed, known]) => {
      setSessions(found.data ?? [])
      setInstallations(installed.data ?? [])
      setProfiles(known.data ?? [])
      setError(found.error)
    })
  }, [project])

  if (!project || (sessions.length === 0 && !error)) return null

  const open = (session: OutsideSession): void => {
    const profile = profileFor(session, installations, profiles)
    if (!profile) {
      setError(t('sessions.resume.noProfile'))
      return
    }
    void ask(() => commands.chatAdopt(project.id, session.sessionId, profile.id, session.title, null)).then((answer) => {
      if (answer.error || !answer.data) return setError(answer.error ?? t('sessions.resume.couldNot'))
      show('chat', { id: answer.data, title: titled(session) })
    })
  }

  return (
    <>
      <div className="heading">{t('sessions.outside.head')}</div>
      {error && <div className="sessions__none">{error}</div>}
      {sessions.slice(0, 10).map((session) => {
        const installation = installations.find((one) => one.directory === session.installation)
        return (
          <button className="card" key={session.sessionId} title={titled(session)} onClick={() => open(session)}>
            <span className="card__l1">
              <span className="card__t">{short(titled(session), 30)}</span>
            </span>
            <span className="card__l2">
              <span className="card__loose">
                {[installation ? named(installation) : null, when(session.lastAt)].filter(Boolean).join(' · ')}
              </span>
            </span>
          </button>
        )
      })}
    </>
  )
}

function when(seconds: number | null): string {
  if (!seconds) return ''
  return formatDate(seconds * 1000, { month: 'short', day: 'numeric' })
}
