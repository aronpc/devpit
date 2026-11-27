import { useCallback, useEffect, useMemo, useState } from 'react'

import type { AgentThread, LiveSession, Project } from '../gen/bindings'
import { formatDate, formatRelative } from '../i18n/format'
import { t } from '../i18n'
import { ask, commands } from './live'
import { refreshSessions, stateWord, useLiveSessions } from './liveStatus'
import { LINKS_CHANGED } from './projectProposal'
import { panelGroups, stateOfOne, useCardTitles } from './sessionsPanel'
import { ReplyDrafts } from './ReplyDrafts'
import { SessionTerminal } from './SessionTerminal'
import { opened } from './strip'
import { remember, remembered } from './tabs'
import { PanelAct, PanelEmpty, PanelHead } from './PanelHead'
import { useShell } from './useShell'
import { EndedSessions } from './EndedSessions'
import { RemoteControlMark } from './RemoteControlMark'
import { SessionCommand } from './SessionCommand'
import { REMOTE_CONTROL } from './sessionCommands'
import { SessionCost } from './SessionCost'
import { Icon } from './Icon'

/*
 * The sessions an orchestrator works with, in one place: how each stands,
 * what can be done with it, and what passed between them.
 *
 * Grouped by project, the ones that need the person first. A row says who and
 * how it is; its two buttons open the session's terminal right here or go to
 * it; opening the row shows the question it waits on, a reply typed into its
 * terminal as the person, and the history of what it was asked and answered.
 * Sessions no longer running stay below, with their history.
 */

const ARROW = { sent: '→', heard: '←', idle: '✓' } as const
const HISTORY = 4

const when = (at: string): string => {
  const date = new Date(at)
  return Number.isNaN(date.getTime()) ? '' : formatDate(date, { dateStyle: 'short', timeStyle: 'short' })
}

/** How long it has been in its state, in words. */
export function since(ms: number | null, now: number): string | null {
  if (ms === null) return null
  const minutes = Math.max(0, Math.round((now - ms) / 60000))
  if (minutes < 1) return t('sessions.justNow')
  if (minutes < 60) return formatRelative(-minutes, 'minute')
  const hours = Math.round(minutes / 60)
  return hours < 24 ? formatRelative(-hours, 'hour') : formatRelative(-Math.round(hours / 24), 'day')
}

/** Whether a stop was refused because it would lose work, and asks again. */
export const wouldLose = (error: string | null): boolean => Boolean(error?.endsWith('stop it anyway?')) // ui-text: ok the core's own English sentence, matched not shown

/** A row's own key: a name started twice is two sessions, and keyed by name
 *  they opened and closed as one. */
export const rowOf = (one: LiveSession): string => `${one.name}:${one.pid}`

/* Whether linked projects with nothing running get a group: a choice kept
   per window, and off until asked for. */
const EMPTY_KEY = 'devpit.sessions.showEmpty'
const keptEmpty = (): boolean => {
  try {
    return localStorage.getItem(EMPTY_KEY) === '1'
  } catch {
    return false
  }
}
const keepEmpty = (on: boolean): void => {
  try {
    localStorage.setItem(EMPTY_KEY, on ? '1' : '0')
  } catch {
    /* Not kept: the panel still works, it forgets on reload. */
  }
}

/* "3 min" has to move while the panel is on screen, and only then. */
function useNow(shown: boolean): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!shown) return
    setNow(Date.now())
    const every = setInterval(() => setNow(Date.now()), 30_000)
    return () => clearInterval(every)
  }, [shown])
  return now
}

export function SessionsView({ shown }: { shown: boolean }): React.JSX.Element {
  const { project, projects, setProject, show, openCard, openPane } = useShell()
  const [linked, setLinked] = useState<readonly string[]>([])
  const profileId = project?.orchestrator ?? null
  const sessions = useLiveSessions(profileId)
  const [threads, setThreads] = useState<readonly AgentThread[]>([])
  const [open, setOpen] = useState<string | null>(null)
  const [terminal, setTerminal] = useState<LiveSession | null>(null)
  const [query, setQuery] = useState('')
  const [showEmpty, setShowEmpty] = useState(keptEmpty)
  /* The row whose Stop is being confirmed, in place. */
  const [stopping, setStopping] = useState<string | null>(null)
  const [said, setSaid] = useState<string | null>(null)
  const now = useNow(shown)
  const cardTitles = useCardTitles(sessions, shown)

  const readHistory = useCallback(() => {
    if (!project?.orchestrator) return
    void ask(() => commands.orchestratorAgents(project.id)).then((answer) => setThreads(answer.data ?? []))
  }, [project])
  const readLinks = useCallback(() => {
    if (project) void ask(() => commands.orchestratorLinks(project.id)).then((answer) => setLinked(answer.data ?? []))
  }, [project])
  useEffect(() => {
    if (!shown) return
    readHistory()
    readLinks()
  }, [shown, readHistory, readLinks])
  /* Linked from the Boards panel or a proposal: this panel's groups move too. */
  useEffect(() => {
    window.addEventListener(LINKS_CHANGED, readLinks)
    return () => window.removeEventListener(LINKS_CHANGED, readLinks)
  }, [readLinks])

  /* A project with a session the orchestrator cannot reach yet, linked in
     one click: its tools act only on linked projects. */
  const link = (id: string): void => {
    if (!project) return
    void ask(() => commands.orchestratorLink(project.id, [...new Set([...linked, id])])).then((answer) => {
      setSaid(answer.error)
      if (answer.error) return
      window.dispatchEvent(new CustomEvent(LINKS_CHANGED))
    })
  }

  /* Asked again when it would lose work: working, or files not committed. */
  const [losing, setLosing] = useState<{ row: string; why: string } | null>(null)
  const stop = (one: LiveSession, force = false): void => {
    if (!profileId) return
    void ask(() => commands.orchestratorStop(profileId, one.name, force)).then((done) => {
      setStopping(null)
      if (wouldLose(done.error)) return setLosing({ row: rowOf(one), why: done.error! })
      setLosing(null)
      setSaid(done.error ?? done.data ?? null)
      refreshSessions(profileId)
    })
  }

  /* A new terminal in a project: a tab of its own there, kept in its strip,
     opened here over the chat. */
  const newTerminal = (one: Project): void => {
    const tabId = crypto.randomUUID()
    void ask(() => commands.sessionEnsure(one.id, tabId, null)).then((made) => {
      if (!made.data) return
      const was = remembered(one.id)
      remember(one.id, { ...opened(was, { id: tabId, kind: 'term', panes: [made.data.focusedId] }), active: was.active ?? tabId })
      setTerminal({
        name: 'Terminal',
        pid: 0,
        job: null,
        status: 'idle',
        kind: 'interactive',
        cwd: one.rootPath,
        projectId: one.id,
        projectName: one.name,
        cardId: null,
        since: null,
        inDevpit: true,
        waiting: null,
        pane: { projectId: one.id, paneId: made.data.focusedId },
        sessionId: null,
        step: null,
        draft: null,
        remote: { on: false, url: null },
      })
    })
  }
  /* A new chat there: its strip gets one, and the project opens on it. */
  const newChat = (one: Project): void => {
    remember(one.id, opened(remembered(one.id), { id: crypto.randomUUID(), kind: 'chat' }))
    setProject(one.id)
  }

  /* A background session attached in a new tab of its project, and that
     terminal opened here over the chat. */
  const watch = (one: LiveSession): void => {
    if (!profileId || !one.job || !one.projectId) return
    const projectId = one.projectId
    void ask(() => commands.sessionWatch(profileId, projectId, one.job!)).then((answer) => {
      if (answer.data) setTerminal({ ...one, inDevpit: true, pane: { projectId, paneId: answer.data } })
    })
  }

  const go = (one: LiveSession): void => {
    if (!one.projectId) return
    setProject(one.projectId)
    if (one.pane) openPane(one.pane.paneId)
    else if (one.cardId) {
      show('board')
      openCard(one.cardId)
    }
  }

  const groups = useMemo(
    () => panelGroups(sessions, linked, projects, query, showEmpty, cardTitles),
    [sessions, linked, projects, query, showEmpty, cardTitles],
  )
  const earlier = threads.filter((thread) => !sessions.some((one) => one.name === thread.name))
  const waiting = sessions.filter((one) => one.waiting).length
  const busy = sessions.filter((one) => one.status === 'busy' && !one.waiting).length
  const historyOf = (name: string): AgentThread | undefined => threads.find((thread) => thread.name === name)

  return (
    <div className="sess">
      <PanelHead
        title={t('sessions.title')}
        meta={
          sessions.length > 0 && (
            <span className="sess__sum">
              {waiting > 0 && <b className="sess__wait">{t('sessions.sum.waiting', { count: waiting })}</b>}
              {busy > 0 && <span>{t('sessions.sum.working', { count: busy })}</span>}
              <span>{t('sessions.sum.running', { count: sessions.length })}</span>
            </span>
          )
        }
      >
        <PanelAct
          label={showEmpty ? t('sessions.hideEmpty') : t('sessions.showEmpty')}
          active={showEmpty}
          onClick={() => setShowEmpty((was) => (keepEmpty(!was), !was))}
        >
          <Icon size="s"><path d="M3 7h18M3 12h18M3 17h10" /></Icon>
        </PanelAct>
        <PanelAct label={t('sessions.readAgain')} onClick={() => (profileId && refreshSessions(profileId), readHistory())}>
          <Icon size="s"><path d="M21 12a9 9 0 1 1-3-6.7L21 8M21 3v5h-5" /></Icon>
        </PanelAct>
      </PanelHead>

      {profileId && <ReplyDrafts profileId={profileId} sessions={sessions} onDone={() => refreshSessions(profileId)} />}

      {sessions.length > 3 && (
        <input className="sess__find" type="search" value={query} placeholder={t('sessions.find.placeholder')} aria-label={t('sessions.find.label')} onChange={(event) => setQuery(event.target.value)} />
      )}
      {said && <p className="sess__note sess__said">{said}</p>}

      {sessions.length === 0 && (
        <PanelEmpty
          title={t('sessions.none.title')}
          hint={
            groups.length > 0
              ? t('sessions.none.hintGroups')
              : t('sessions.none.hint')
          }
        />
      )}

      {sessions.length > 0 && groups.length === 0 && <p className="sess__note">{t('sessions.noMatch', { query })}</p>}

      {groups.map(({ project: where, members, linked: reached }) => (
        <section className="sess__group" key={where?.id ?? 'outside'}>
          <div className="sess__gh">
            <h3 className="sess__g">{where?.name ?? members[0]?.projectName ?? t('sessions.outside')}</h3>
            {where && !reached && (
              <button className="sess__link" onClick={() => link(where.id)} title={t('sessions.link.title', { name: where.name })}>
                {t('sessions.link')}
              </button>
            )}
            {where && (
              <>
                <button className="sess__icon" onClick={() => newTerminal(where)} title={t('sessions.newTerminal.title', { name: where.name })} aria-label={t('sessions.newTerminal', { name: where.name })}>
                  <Icon size="s"><path d="m5 8 4 4-4 4M12 16h7" /></Icon>
                </button>
                <button className="sess__icon" onClick={() => newChat(where)} title={t('sessions.newChat', { name: where.name })} aria-label={t('sessions.newChat', { name: where.name })}>
                  <Icon size="s"><path d="M21 11.5a8.4 8.4 0 0 1-9 8.4 9.9 9.9 0 0 1-4.2-.9L3 20.5l1.6-4.4A8.4 8.4 0 0 1 3.6 11.5 8.4 8.4 0 0 1 12 3.1a8.4 8.4 0 0 1 9 8.4Z" /></Icon>
                </button>
              </>
            )}
          </div>
          {members.length === 0 && <p className="sess__none">{t('sessions.nothingHere')}</p>}
          {members.map((one) => {
            const state = stateOfOne(one)
            const card = one.cardId ? cardTitles.get(one.cardId) ?? t('sessions.aCard') : null
            const history = historyOf(one.name)
            const last = history?.events[history.events.length - 1]
            const expanded = open === rowOf(one)
            return (
              <div className="sess__one" key={rowOf(one)} data-state={state} data-open={expanded ? 'true' : undefined}>
                <div className="sess__row">
                  <button className="sess__main" aria-expanded={expanded} onClick={() => setOpen(expanded ? null : rowOf(one))} title={one.cwd}>
                    <i className="deleg__dot" data-state={state} />
                    <span className="sess__name">{one.name}</span>
                    <span className="sess__state">{stateWord(state)}</span>
                    <span className="sess__meta">
                      {[card, since(one.since, now)].filter(Boolean).join(' · ')}
                      {/* What it is on right now beats what it last said: a
                          working session's last reply is from the turn before. */}
                      {one.step && state === 'busy'
                        ? ` · ${one.step}`
                        : last && ` · ${ARROW[last.kind as keyof typeof ARROW] ?? ''} ${last.summary ?? last.text}`}
                    </span>
                  </button>
                  <SessionCost sessionId={one.sessionId} />
                  <RemoteControlMark remote={one.remote} name={one.name} />
                  {one.pane && (
                    <button className="sess__icon" onClick={() => setTerminal(one)} title={t('sessions.openTerminal.title')} aria-label={t('sessions.openTerminal', { name: one.name })}>
                      <Icon size="s"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="m7 10 3 2-3 2M13 14h4" /></Icon>
                    </button>
                  )}
                  {one.projectId && (
                    <button className="sess__icon" onClick={() => go(one)} title={t('sessions.go.title')} aria-label={t('sessions.go', { name: one.name })}>
                      <Icon size="s"><path d="M7 17 17 7M8 7h9v9" /></Icon>
                    </button>
                  )}
                  <button className="sess__icon sess__icon--stop" onClick={() => setStopping(rowOf(one))} title={t('sessions.stop.title')} aria-label={t('sessions.stop', { name: one.name })}>
                    <Icon size="s"><rect x="6" y="6" width="12" height="12" rx="2" /></Icon>
                  </button>
                </div>
                {stopping === rowOf(one) && (
                  <div className="sess__acts sess__stopask">
                    <span className="sess__warn">{t('sessions.stop.ask', { name: one.name })}</span>
                    <button className="sess__btn" onClick={() => setStopping(null)}>{t('sessions.keep')}</button>
                    <button className="sess__btn sess__btn--bad" onClick={() => stop(one)}>{t('sessions.stopNow')}</button>
                  </div>
                )}
                {losing?.row === rowOf(one) && (
                  <div className="sess__acts sess__stopask">
                    <span className="sess__warn">{losing.why}</span>
                    <button className="sess__btn" onClick={() => setLosing(null)}>{t('sessions.keep')}</button>
                    <button className="sess__btn sess__btn--bad" onClick={() => stop(one, true)}>{t('sessions.stopAnyway')}</button>
                  </div>
                )}
                {expanded && (
                  <Details
                    session={one}
                    history={history}
                    profileId={profileId}
                    onWatch={one.job && one.projectId && !one.pane ? () => watch(one) : undefined}
                    onTerminal={one.pane ? () => setTerminal(one) : undefined}
                  />
                )}
              </div>
            )
          })}
        </section>
      ))}

      {profileId && <EndedSessions profileId={profileId} running={sessions.length} />}

      {earlier.length > 0 && (
        <section className="sess__group">
          <h3 className="sess__g">{t('sessions.earlier')}</h3>
          {earlier.map((thread) => (
            <div className="sess__one" key={thread.name} data-state="gone" data-open={open === thread.name ? 'true' : undefined}>
              <div className="sess__row">
                <button className="sess__main" aria-expanded={open === thread.name} onClick={() => setOpen(open === thread.name ? null : thread.name)}>
                  <i className="deleg__dot" data-state="gone" />
                  <span className="sess__name">{thread.name}</span>
                  <span className="sess__state">{when(thread.lastAt)}</span>
                </button>
              </div>
              {open === thread.name && <History thread={thread} />}
            </div>
          ))}
        </section>
      )}

      {terminal && (
        <SessionTerminal
          session={terminal}
          profileId={profileId}
          onClose={() => setTerminal(null)}
          onGo={() => {
            setTerminal(null)
            go(terminal)
          }}
        />
      )}
    </div>
  )
}


function Details({
  session,
  history,
  profileId,
  onWatch,
  onTerminal,
}: {
  session: LiveSession
  history: AgentThread | undefined
  profileId: string | null
  /** A background session, opened in a terminal to be watched and typed into. */
  onWatch?: () => void
  /** Its own terminal, opened over the panel. */
  onTerminal?: () => void
}): React.JSX.Element {
  const [reply, setReply] = useState('')
  const [said, setSaid] = useState<string | null>(null)
  /* Stopping asks once, in place: work in flight is lost. */
  const [stopping, setStopping] = useState(false)

  const [losing, setLosing] = useState<string | null>(null)
  const stop = (force = false): void => {
    if (!profileId) return
    void ask(() => commands.orchestratorStop(profileId, session.name, force)).then((done) => {
      setStopping(false)
      if (wouldLose(done.error)) return setLosing(done.error)
      setLosing(null)
      setSaid(done.error ?? done.data ?? null)
      refreshSessions(profileId)
    })
  }

  /* Typed into that session's own terminal, as the person: a message from
     the orchestrator would approve nothing there, and must not. */
  const send = (): void => {
    const text = reply.trim()
    if (!text || !profileId) return
    void ask(() => commands.orchestratorReply(profileId, session.name, text, 'the Sessions panel')).then((sent) => {
      setSaid(sent.error ?? t('sessions.sentAsYou'))
      if (sent.error) return
      setReply('')
      refreshSessions(profileId)
    })
  }

  /* Typed in the session's own terminal as the person: the orchestrator types
     into nobody's. Remote Control's page and code show there, so it opens. */
  const command = (line: string): void => {
    if (!profileId) return
    void ask(() => commands.orchestratorReply(profileId, session.name, line, 'the Sessions panel')).then((sent) => {
      setSaid(sent.error ?? (line === REMOTE_CONTROL ? t('sessions.typedRemote', { command: REMOTE_CONTROL }) : t('sessions.typedAsYou', { command: line })))
      if (!sent.error && line === REMOTE_CONTROL) onTerminal?.()
    })
  }

  return (
    <div className="sess__more">
      {session.waiting && (
        <p className="sess__q">
          <b>{t('sessions.waitingOnYou')}</b> {session.waiting.question}
        </p>
      )}
      {session.inDevpit ? (
        <form className="sess__reply" onSubmit={(event) => (event.preventDefault(), send())}>
          <input value={reply} maxLength={4000} placeholder={t('sessions.reply.placeholder', { name: session.name })} aria-label={t('sessions.reply.label', { name: session.name })} onChange={(event) => setReply(event.target.value)} />
        </form>
      ) : onWatch ? (
        <p className="sess__note">
          {t('sessions.background')}{' '}
          <button className="sess__more-btn" onClick={onWatch}>
            {t('sessions.watch')}
          </button>
        </p>
      ) : (
        <p className="sess__note">{t('sessions.outsideNote')}</p>
      )}
      {said && <p className="sess__note">{said}</p>}
      <div className="sess__acts">
        {stopping ? (
          <>
            <span className="sess__warn">{t('sessions.stop.askHere')}</span>
            <button className="sess__btn" onClick={() => setStopping(false)}>{t('sessions.keep')}</button>
            <button className="sess__btn sess__btn--bad" onClick={() => stop()}>{t('sessions.stopNow')}</button>
          </>
        ) : losing ? (
          <>
            <span className="sess__warn">{losing}</span>
            <button className="sess__btn" onClick={() => setLosing(null)}>{t('sessions.keep')}</button>
            <button className="sess__btn sess__btn--bad" onClick={() => stop(true)}>{t('sessions.stopAnyway')}</button>
          </>
        ) : (
          <>
            {session.pane && !session.waiting && <SessionCommand name={session.name} onSend={command} />}
            <button className="sess__btn" onClick={() => setStopping(true)} aria-label={t('sessions.stop', { name: session.name })}>
              {t('sessions.stopSession')}
            </button>
          </>
        )}
      </div>
      {history ? <History thread={history} /> : <p className="sess__note">{t('sessions.noHistory')}</p>}
    </div>
  )
}

function History({ thread }: { thread: AgentThread }): React.JSX.Element {
  const [all, setAll] = useState(false)
  const events = all ? thread.events : thread.events.slice(-HISTORY)
  return (
    <div className="sess__hist">
      {thread.events.length > HISTORY && !all && (
        <button className="sess__more-btn" onClick={() => setAll(true)}>
          {t('sessions.showAll', { count: thread.events.length })}
        </button>
      )}
      <ol>
        {events.map((event, at) => (
          <li key={at} data-kind={event.kind}>
            <span className="sess__when">
              {ARROW[event.kind as keyof typeof ARROW] ?? '·'} {when(event.at)}
            </span>
            {event.summary && <strong>{event.summary}</strong>}
            <span className="sess__text">{event.kind === 'idle' ? t('sessions.finishedTurn', { text: event.text }) : event.text}</span>
          </li>
        ))}
      </ol>
    </div>
  )
}
