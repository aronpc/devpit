import { useCallback, useEffect, useRef, useState } from 'react'

import type { Installation, OutsideSession, Profile, Thread } from '../gen/bindings'
import { ask, commands } from './live'
import { profileFor, scopeOf, sessionInScope, threadInScope, titled, type Scope } from './outside'
import { abandoned, committed } from './typing'
import { named } from './useInstallations'
import { useShell } from './useShell'
import { intoView } from './intoView'
import { t } from '../i18n'
import { formatDate } from '../i18n/format'

/*
 * `/resume` in a chat: every earlier conversation of this project, to pick
 * one up where it stopped — listed where the slash menu opens, above the
 * composer, the way the terminal lists them in place rather than in a dialog.
 *
 * Both kinds the sidebar lists apart — devpit's own chats, and sessions
 * started in a terminal — in one list, newest first, because from the
 * composer the question is "which conversation", not "where was it begun".
 * A terminal session is adopted the way the sidebar adopts one, under the
 * profile that runs against the installation that wrote it.
 *
 * Narrowed to the account the chat is on, because each account keeps its own
 * history: `claude2`'s conversations are not in `~/.claude`. One click widens
 * it, for the conversation somebody remembers having on another account.
 */

interface Row {
  readonly key: string
  readonly title: string
  readonly at: number
  readonly where: string
  readonly open: () => Promise<string | null>
}

export function ResumePicker({
  from,
  profileId,
  onClose,
}: {
  from: string
  /** The account the chat is on, or null before one is picked. */
  profileId: string | null
  onClose: () => void
}): React.JSX.Element {
  const { project, replace } = useShell()
  /* In this tab's place: `/resume` was typed here, so here is where it goes. */
  const show = useCallback((id: string, title: string | undefined) => replace(from, { id, kind: 'chat', title }), [replace, from])
  const [wanted, setWanted] = useState('')
  const [at, setAt] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [every, setEvery] = useState(false)
  const field = useRef<HTMLInputElement>(null)

  /* Each kind as it arrives: devpit's own threads are a file read, while the
     terminal's sessions are a scan of the CLI's transcripts that can take a
     while — waiting on both left the list saying "Reading" over rows that had
     long been ready. */
  const [threads, setThreads] = useState<readonly Thread[] | null>(null)
  const [sessions, setSessions] = useState<readonly OutsideSession[] | null>(null)
  const [installations, setInstallations] = useState<readonly Installation[]>([])
  const [profiles, setProfiles] = useState<readonly Profile[]>([])
  useEffect(() => {
    if (!project) return
    void Promise.all([ask(() => commands.chatList(project.id)), ask(() => commands.agentProfiles())]).then(
      ([listed, known]) => {
        setThreads(listed.data?.conversations ?? [])
        setProfiles(known.data ?? [])
        if (listed.error) setError(listed.error)
      },
    )
    void Promise.all([ask(() => commands.chatOutside(project.id)), ask(() => commands.cliInstallations())]).then(
      ([found, installed]) => {
        setSessions(found.data ?? [])
        setInstallations(installed.data ?? [])
        if (found.error) setError(found.error)
      },
    )
  }, [project])

  const scope: Scope | null = every ? null : scopeOf(profileId, profiles, installations)
  const account = profiles.find((one) => one.id === profileId)?.label
  const rows: Row[] | null =
    threads === null && sessions === null
      ? null
      : [
          ...(threads ?? [])
            .filter((thread) => threadInScope(thread.profile, scope, profiles, installations))
            .map((thread) => ownRow(thread, profiles, show)),
          ...(sessions ?? [])
            .filter((session) => sessionInScope(session.installation, scope))
            .map((session) => outsideRow(session, project?.id ?? '', installations, profiles, show)),
        ].sort((a, b) => b.at - a.at)
  const reading = threads === null || sessions === null

  /* Not the conversation already on screen. */
  const shown = (rows ?? []).filter((row) => row.key !== from && row.title.toLowerCase().includes(wanted.trim().toLowerCase()))
  const pick = (row: Row | undefined): void => {
    if (!row) return
    void row.open().then((failed) => (failed ? setError(failed) : onClose()))
  }

  return (
    <div className="slash resume" role="dialog" aria-label={t('sessions.resume.label')}>
      <div className="resume__in">
        <div className="resume__top">
          <input
            ref={field}
            className="resume__find"
            autoFocus
            placeholder={t('sessions.resume.placeholder')}
            value={wanted}
            onChange={(event) => {
              setWanted(event.target.value)
              setAt(0)
            }}
            onBlur={(event) => {
              /* Away from the list — a click elsewhere — is a no. */
              if (!event.currentTarget.closest('.resume')?.contains(event.relatedTarget as Node | null)) onClose()
            }}
            onKeyDown={(event) => {
              if (abandoned(event)) onClose()
              if (committed(event)) pick(shown[at])
              if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
                event.preventDefault()
                const step = event.key === 'ArrowDown' ? 1 : -1
                setAt((was) => Math.max(0, Math.min(shown.length - 1, was + step)))
              }
            }}
          />
          {account && (
            <button
              className="resume__scope"
              aria-pressed={every}
              title={every ? t('sessions.resume.onlyAccount', { account }) : t('sessions.resume.everyTitle')}
              onClick={() => {
                setEvery((was) => !was)
                setAt(0)
                /* Back to the field: the arrows and Escape live there. */
                field.current?.focus()
              }}
            >
              {every ? t('sessions.resume.every') : account}
            </button>
          )}
        </div>
        {error && <p className="acc__note">{error}</p>}
        <div className="resume__list" role="listbox">
          {shown.length === 0 && (
            <div className="resume__none">
              {reading
                ? t('sessions.resume.reading')
                : scope
                  ? t('sessions.resume.noneOn', { account: account ?? t('sessions.resume.thisAccount') })
                  : t('sessions.resume.none')}
            </div>
          )}
          {shown.map((row, index) => (
            <button
              key={row.key}
              className="resume__o"
              role="option"
              aria-selected={index === at}
              ref={index === at ? intoView : undefined}
              onMouseEnter={() => setAt(index)}
              onClick={() => pick(row)}
            >
              <span className="resume__t">{row.title}</span>
              <span className="resume__m">{[row.where, when(row.at)].filter(Boolean).join(' · ')}</span>
            </button>
          ))}
          {reading && shown.length > 0 && <div className="resume__none">{t('sessions.resume.stillReading')}</div>}
        </div>
      </div>
    </div>
  )
}

type Show = (id: string, title: string | undefined) => void

function ownRow(thread: Thread, profiles: readonly Profile[], show: Show): Row {
  return {
    key: thread.id,
    title: thread.title,
    at: thread.lastAt ?? 0,
    /* The name somebody gave the account, not the id minted for it. */
    where: profiles.find((one) => one.id === thread.profile)?.label ?? (thread.profile || 'devpit'),
    open: () => {
      show(thread.id, thread.title)
      return Promise.resolve(null)
    },
  }
}

function outsideRow(
  session: OutsideSession,
  projectId: string,
  installations: readonly Installation[],
  profiles: readonly Profile[],
  show: Show,
): Row {
  const installation = installations.find((one) => one.directory === session.installation)
  return {
    key: `outside:${session.sessionId}`,
    title: titled(session),
    at: session.lastAt ?? 0,
    where: installation ? t('sessions.resume.terminalOf', { name: named(installation) }) : t('sessions.resume.terminal'),
    open: async () => {
      const profile = profileFor(session, installations, profiles)
      if (!profile) return t('sessions.resume.noProfile')
      const answer = await ask(() => commands.chatAdopt(projectId, session.sessionId, profile.id, session.title, null))
      if (answer.error || !answer.data) return answer.error ?? t('sessions.resume.couldNot')
      show(answer.data, titled(session))
      return null
    },
  }
}

function when(seconds: number): string {
  if (!seconds) return ''
  return formatDate(seconds * 1000, { month: 'short', day: 'numeric' })
}
