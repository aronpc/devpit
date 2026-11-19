import { useEffect, useState } from 'react'

import type { EndedSession, LiveSession } from '../gen/bindings'
import { endedHow } from './EndedSessions'
import { ask, commands } from './live'
import { refreshSessions, stateWord, useLiveSessions } from './liveStatus'
import { endedLatestFirst, needingYou, roundOf, toResume } from './orchestratorShortcuts'
import { stateOfOne } from './sessionsPanel'
import type { OwnAsked } from './slash'
import { t } from '../i18n'
import { formatDate } from '../i18n/format'

/*
 * An orchestrator chat's shortcut, answered above the composer from what
 * devpit already reads about the account's sessions: at once, without a turn,
 * and kept current while it is open — the same live list the Sessions panel
 * draws, so the two never disagree.
 */

export function OrchestratorAnswer({ asked, profileId, onClose }: { asked: OwnAsked; profileId: string; onClose: () => void }): React.JSX.Element {
  const live = useLiveSessions(profileId)
  const [ended, setEnded] = useState<readonly EndedSession[] | null>(null)
  const [said, setSaid] = useState<string | null>(null)
  /* The `/resume-session <session>` already resumed: once per asking, not per render. */
  const [resumed, setResumed] = useState<OwnAsked | null>(null)

  /* Read again when one more or one fewer is running: a session ended or resumed. */
  useEffect(() => {
    if (asked.name === 'waiting') return
    void ask(() => commands.orchestratorEnded(profileId)).then((answer) => setEnded(answer.data?.sessions ?? []))
  }, [asked, profileId, live.length])

  /* What a resume said belongs to the shortcut it was asked from. */
  useEffect(() => setSaid(null), [asked])

  const resume = (one: EndedSession): void => {
    setSaid(t('sessions.ans.resuming', { name: one.name }))
    void ask(() => commands.orchestratorResume(profileId, one.sessionId)).then((answer) => {
      setSaid(answer.error ?? t('sessions.ended.resumed', { name: answer.data ?? one.name, project: one.projectName ?? t('sessions.ended.itsProject') }))
      if (!answer.error) refreshSessions(profileId)
    })
  }

  /* `/resume-session <session>` naming exactly one resumes it; naming more, the person picks. */
  const matches = asked.name === 'resume-session' && ended ? toResume(ended, asked.arg) : null
  const only = asked.arg && matches?.length === 1 ? matches[0]! : null
  useEffect(() => {
    if (!only || resumed === asked) return
    setResumed(asked)
    resume(only)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [only, asked, resumed])

  return (
    <section className="slash oans" aria-label={`/${asked.name}`}>
      <div className="oans__top">
        <p className="oans__t">/{[asked.name, asked.arg].filter(Boolean).join(' ')}</p>
        <button className="sq26" onClick={onClose} aria-label={t('sessions.ans.close')} title={t('sessions.ans.close')}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
        </button>
      </div>
      {asked.name === 'sessions' && <AllSessions live={live} ended={ended} />}
      {asked.name === 'waiting' && <Rows live={needingYou(live)} none={t('sessions.ans.noneWaiting')} />}
      {asked.name === 'ended' && <Ended ended={ended === null ? null : endedLatestFirst(ended)} none={t('sessions.ans.noneEnded')} onResume={resume} />}
      {asked.name === 'resume-session' && resumed !== asked && !only && (
        <>
          {asked.arg && matches?.length === 0 && <p className="sess__note">{t('sessions.ans.noCalled', { name: asked.arg })}</p>}
          <Ended ended={matches === null ? null : matches.length > 0 ? matches : toResume(ended ?? [], '')} none={t('sessions.ans.noneResumable')} onResume={resume} />
        </>
      )}
      {said && <p className="sess__note">{said}</p>}
    </section>
  )
}

function AllSessions({ live, ended }: { live: readonly LiveSession[]; ended: readonly EndedSession[] | null }): React.JSX.Element {
  const round = roundOf(live, ended ?? [])
  const groups: [string, readonly LiveSession[]][] = [
    [t('sessions.ans.waiting'), round.waiting],
    [t('sessions.ans.working'), round.working],
    [t('sessions.ans.idle'), round.idle],
  ]
  return (
    <>
      {live.length === 0 && <p className="sess__note">{t('sessions.ans.nothingRunning')}</p>}
      {groups
        .filter(([, members]) => members.length > 0)
        .map(([title, members]) => (
          <div className="oans__g" key={title}>
            <p className="oans__h">
              {title} · {members.length}
            </p>
            <Rows live={members} />
          </div>
        ))}
      {round.ended.length > 0 && (
        <div className="oans__g">
          <p className="oans__h">{t('sessions.ans.endedLately')}</p>
          <Ended ended={round.ended} />
        </div>
      )}
    </>
  )
}

function Rows({ live, none }: { live: readonly LiveSession[]; none?: string }): React.JSX.Element {
  if (live.length === 0) return none ? <p className="sess__note">{none}</p> : <></>
  return (
    <>
      {live.map((one) => (
        <div className="oans__i" key={`${one.name}:${one.pid}`} data-state={stateOfOne(one)}>
          <span className="oans__n">{one.name}</span>
          <span className="oans__w">{[one.projectName ?? t('sessions.ans.outside'), one.step].filter(Boolean).join(' · ')}</span>
          <span className="oans__s">{one.waiting ? stateWord('waiting') : one.draft ? t('sessions.ans.draft') : stateWord(stateOfOne(one))}</span>
          {one.waiting && <p className="oans__q">{one.waiting.question}</p>}
        </div>
      ))}
    </>
  )
}

function Ended({ ended, none, onResume }: { ended: readonly EndedSession[] | null; none?: string; onResume?: (one: EndedSession) => void }): React.JSX.Element {
  if (ended === null) return <p className="sess__note">{t('sessions.ans.reading')}</p>
  if (ended.length === 0) return none ? <p className="sess__note">{none}</p> : <></>
  return (
    <>
      {ended.map((one) => (
        <div className="oans__i" key={one.sessionId} data-state="gone">
          <span className="oans__n">{one.name}</span>
          <span className="oans__w">
            {[one.projectName, one.endedAt === null ? null : formatDate(new Date(one.endedAt), { dateStyle: 'short', timeStyle: 'short' })].filter(Boolean).join(' · ')}
          </span>
          <span className="oans__s">{endedHow(one)}</span>
          {onResume && one.projectId && (
            <button className="sess__btn" onClick={() => onResume(one)} title={t('sessions.ended.resume.title')}>
              {t('sessions.ended.resume')}
            </button>
          )}
        </div>
      ))}
    </>
  )
}
