import { useEffect, useRef, useState } from 'react'

import { AgentMark } from './AgentMark'
import { ask, commands } from './live'
import { PANES } from './paneList'
import { since } from './projects'
import { Rename } from './Rename'
import { busyIn, doingIn } from './running'
import { grouped } from './settling'
import { short, twice, type Clicked, type Tab } from './strip'
import { modelName, runningIn } from './subagents'
import { unreadIn } from './unread'
import { useShell } from './useShell'
import { t } from '../i18n'

/*
 * The sessions that are open, and only those.
 *
 * Six fixed rows used to live here, naming work nobody was doing and each one
 * wired to "open a chat" rather than to the session it claimed to be. Every
 * button had a handler, so the dead-control guard let it through — a control
 * can lie without being dead.
 */

export function SessionRows(): React.JSX.Element {
  const {
    open,
    active,
    focus,
    close,
    rename,
    renaming,
    setRenaming,
    running,
    doing,
    unread,
    subagents,
    project,
    threads,
    profiles,
    chatDoing,
    mark,
  } = useShell()
  const clicked = useRef<Clicked | null>(null)
  const now = useMinute()
  const sessions = open.filter((tab) => tab.kind === 'term' || tab.kind === 'chat')
  const thread = (tab: Tab) => (tab.kind === 'chat' ? threads.find((one) => one.id === tab.id) : undefined)
  const { progress, settled } = grouped(sessions, (tab) => thread(tab)?.marks, now)
  const branches = useBranches(sessions.flatMap((tab) => (tab.kind === 'chat' && project ? [thread(tab)?.cwd ?? project.rootPath] : [])))

  const row = (tab: Tab): React.JSX.Element => {
    const name = tab.title ?? (tab.kind === 'term' ? t('sessions.rows.terminalName') : t('sessions.rows.chat'))
    const talk = thread(tab)
    const profile = talk ? profiles.find((one) => one.id === talk.profile) : undefined
    /* What the pane actually has in front of it, when that is not the
       shell. A terminal with an agent open is a conversation someone is
       having, and the row saying `terminal` would be hiding it. */
    const here = busyIn(running, tab)
    /* What the agent says it is doing, when it was started from here and
       has said anything. `waiting` is the one worth a glance across the
       room: nothing moves until somebody goes back to it. */
    const marked = unreadIn(unread, tab) || Boolean(talk?.marks?.unread)
    const says = tab.kind === 'chat' ? (chatDoing[tab.id] ?? (marked ? 'done' : null)) : doingIn(running, doing, tab)
    const unseen = says === 'done' && marked
    const who = here?.label ?? profile?.label ?? t('sessions.rows.chat')
    /* Where it runs and when it was last spoken in: the facts a row is told apart by. */
    const facts = [talk && branches[talk.cwd ?? project?.rootPath ?? ''], project?.name, talk?.lastAt ? since(talk.lastAt, now * 1000) : null].filter(Boolean).join(' · ')
    const children = here?.agent ? runningIn(subagents, tab.panes) : []
    const subs = children.length > 0 && (
      <span className="card__subs">
        {children.map((child) => (
          <span className="card__sub" key={child.id}>
            <i className="card__dot" />
            <span className="card__subname">{child.name ?? child.kind ?? t('sessions.rows.subagent')}</span>
            {child.model && <span className="card__model">{modelName(child.model)}</span>}
          </span>
        ))}
      </span>
    )
    const below = (
      <span className="card__l2">
        <span className="card__kind" title={profile?.label}>
          {here?.agent || profile ? (
            <AgentMark agent={here?.agent ?? profile?.driver ?? ''} size={13} />
          ) : (
            PANES.find((pane) => pane.name === tab.kind)?.icon
          )}
        </span>
        {here || says ? (
          <span
            className="card__run"
            data-agent={here ? here.agent !== null : true}
            data-doing={says}
            data-unread={unseen || undefined}
          >
            {/* Colour alone does not carry a question to everyone. */}
            {says === 'waiting' ? (
              <i className="card__ask" aria-hidden="true">
                ?
              </i>
            ) : (
              <i className="card__dot" />
            )}
            {says === 'waiting' ? t('sessions.rows.waiting', { who }) : unseen ? t('sessions.rows.finished', { who }) : here ? here.label : t('sessions.rows.working', { who })}
          </span>
        ) : (
          <span className="card__loose">{tab.kind === 'term' ? t('sessions.rows.terminal') : (profile?.label ?? t('sessions.rows.chatLower'))}</span>
        )}
      </span>
    )
    const where = facts && (
      <span className="card__l2">
        <span className="card__loose card__facts">{facts}</span>
      </span>
    )
    /* Renaming swaps the row for a field: an input inside a button is
       neither valid nor operable — the button swallows the click that
       would place the cursor. */
    if (renaming?.id === tab.id && renaming.where === 'sidebar') {
      return (
        <div className="card" key={tab.id}>
          <span className="card__l1">
            <Rename
              className="card__t"
              value={name}
              editing
              onDone={(title) => {
                if (title) rename(tab.id, title)
                if (title && tab.kind === 'chat') mark(tab.id, { name: title })
                setRenaming(null)
              }}
            />
          </span>
          {below}
          {where}
          {subs}
        </div>
      )
    }
    return (
      <button
        className="card"
        data-ctx="session"
        data-id={tab.id}
        data-unread={unseen || undefined}
        key={tab.id}
        aria-pressed={active?.id === tab.id}
        onClick={(event) => {
          /* Two quick clicks rename. Read from the clicks because the row
             is a button that also focuses on the first of them. */
          if (twice(clicked.current, tab.id, event.timeStamp)) {
            clicked.current = null
            setRenaming({ id: tab.id, where: 'sidebar' })
            return
          }
          clicked.current = { id: tab.id, at: event.timeStamp }
          focus(tab.id)
        }}
        onAuxClick={(event) => event.button === 1 && close(tab.id)}
      >
        <span className="card__l1">
          <span className="card__t">{short(name, 30)}</span>
          {talk?.marks?.pinned && (
            <svg className="card__pin" aria-label={t('sessions.rows.pinned')} width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M15 3l6 6-3 1-4 4 1 5-2 2-4-4-5 5-1-1 5-5-4-4 2-2 5 1 4-4z" /></svg>
          )}
        </span>
        {below}
        {where}
        {subs}
      </button>
    )
  }

  return (
    <>
      <div className="heading">
        {t('sessions.rows.inProgress')} <span className="heading__n">{progress.length}</span>
      </div>
      {progress.length === 0 && <div className="sessions__none">{t('sessions.rows.none')}</div>}
      {progress.map(row)}
      {settled.length > 0 && (
        <div className="heading">
          {t('sessions.rows.settled')} <span className="heading__n">{settled.length}</span>
        </div>
      )}
      {settled.map(row)}
    </>
  )
}

/* Unix seconds, a minute at a time: what ends a snooze and ages a row. */
function useMinute(): number {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000))
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Math.floor(Date.now() / 1000)), 60_000)
    return () => window.clearInterval(timer)
  }, [])
  return now
}

/* Each folder's branch, asked once: a row redrawn every second must not ask git again. */
function useBranches(folders: readonly string[]): Readonly<Record<string, string>> {
  const [branches, setBranches] = useState<Record<string, string>>({})
  const asked = useRef(new Set<string>())
  const wanted = [...new Set(folders)].join('\n')
  useEffect(() => {
    for (const folder of wanted.split('\n').filter((one) => one && !asked.current.has(one))) {
      asked.current.add(folder)
      void ask(() => commands.folderGlance(folder)).then((answer) => setBranches((was) => ({ ...was, [folder]: answer.data?.branch ?? '' })))
    }
  }, [wanted])
  return branches
}
