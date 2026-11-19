import type { IslandSession, IslandStep, IslandVerdict } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from '../shell/live'
import { SessionCost } from '../shell/SessionCost'
import { Checks } from './Checks'
import { Mascot } from './Mascot'
import { Preview } from './Preview'
import { Reply } from './Reply'
import { ROWS } from './shape'
import { askedWords, headline, labelled, nameOf, nowWords, stepWords, type Ask, type Mood } from './sessions'

/*
 * The island's views: the capsule, the list of sessions, one session, and a
 * question.
 *
 * Each is drawn to the size `shape.ts` gives it and no bigger; the outline is
 * animated around them, and a view that grew past it would be cut.
 */

/** Brings devpit forward on the terminal a session runs in, or on its chat. */
export const openPane = (session: IslandSession): void => {
  if (!session.paneId) {
    void ask(() => commands.islandOpenChat(session.sessionId))
    return
  }
  void ask(() => commands.islandOpenPane(session.projectId, session.paneId!))
}

/** How long ago, in the fewest characters that still say it. */
export function ago(now: number, at: number | null): string {
  const seconds = Math.max(0, Math.round((now - (at ?? now)) / 1000))
  if (seconds < 45) return t('island.ago.now')
  if (seconds < 3600) return t('island.ago.minutes', { count: Math.round(seconds / 60) })
  return t('island.ago.hours', { count: Math.round(seconds / 3600) })
}

const STATE_KEY: Readonly<Record<IslandSession['state'], string>> = {
  open: 'idle',
  working: 'working',
  waiting: 'waiting',
  done: 'done',
  failed: 'failed',
  gone: 'gone',
}

const stateWord = (state: IslandSession['state']): string => t(`island.state.${STATE_KEY[state]}`)

/** What the head says for a session: where it runs, what it is doing, since when. */
export const whereOf = (session: IslandSession, now: number): string =>
  [session.card ? session.project : null, stateWord(session.state), ago(now, session.at)].filter(Boolean).join(' · ')

export function Capsule({
  sessions,
  questions,
  mood,
  now,
}: {
  sessions: readonly IslandSession[]
  questions: readonly Ask[]
  mood: Mood
  now: number
}): React.JSX.Element {
  const top = sessions[0]
  const labels = labelled(sessions)
  const name = questions[0] ? t('island.capsule.asksTo') : top ? (labels.get(top.sessionId) ?? nameOf(top)) : 'devpit'
  const words = questions[0] ? askedWords(questions[0]) : top ? nowWords(top, now) : t('island.capsule.nothing')
  const busy = sessions.some((one) => one.state === 'working')
  return (
    <div className="isl-cap">
      <Mascot mood={mood} size={26} />
      <span className="isl-cap__text">
        <span className="isl-cap__name">{name}</span>
        <span className="isl-cap__now" data-moving={mood === 'working' || mood === 'thinking' ? 'true' : undefined}>
          {words}
        </span>
      </span>
      <span className="isl-cap__dots" title={headline(sessions, questions)}>
        {sessions.slice(0, 5).map((one) => (
          <i className="isl-cap__dot" data-state={one.state} key={one.sessionId} />
        ))}
        {sessions.length > 5 && <span className="isl-cap__more">+{sessions.length - 5}</span>}
      </span>
      {busy && <span className="isl-cap__run" />}
    </div>
  )
}

export function Rows({
  sessions,
  now,
  asking,
  onChoose,
}: {
  sessions: readonly IslandSession[]
  now: number
  /** The sessions with a question waiting on the person. */
  asking?: ReadonlySet<string>
  onChoose: (session: IslandSession) => void
}): React.JSX.Element {
  const labels = labelled(sessions)
  if (sessions.length === 0) return <div className="isl-rows__none">{t('island.rows.none')}</div>
  return (
    <div className="isl-rows">
      {sessions.slice(0, ROWS).map((session) => (
        <button
          className="isl-row"
          key={session.sessionId}
          data-session={session.sessionId}
          style={{ '--tint': session.color ?? 'var(--accent)' } as React.CSSProperties}
          onClick={() => onChoose(session)}
        >
          <span className="isl-row__bar" />
          <span className="isl-row__main">
            <span className="isl-row__name">{labels.get(session.sessionId) ?? nameOf(session)}</span>
            <span className="isl-row__now">{nowWords(session, now)}</span>
          </span>
          {asking?.has(session.sessionId) && (
            <span className="isl-row__badge" title={t('island.rows.asks')}>
              {t('island.rows.badge')}
            </span>
          )}
          <span className="isl-row__state" data-state={session.state}>
            {stateWord(session.state)}
          </span>
          <span className="isl-row__ago">{ago(now, session.at)}</span>
        </button>
      ))}
    </div>
  )
}

export function Detail({ session, now }: { session: IslandSession; now: number }): React.JSX.Element {
  const steps = session.steps.slice(-6)
  const shown: IslandStep | null = [...session.steps].reverse().find((one) => one.touch) ?? session.steps.at(-1) ?? null
  return (
    <div className="isl-detail" data-session={session.sessionId}>
      <div className="isl-detail__side">
        <ol className="isl-line">
          {steps.length === 0 && (
            <li className="isl-line__step" data-state="idle">
              <span className="isl-line__node" />
              <span className="isl-line__words">{nowWords(session, now)}</span>
            </li>
          )}
          {steps.map((step, at) => (
            <li className="isl-line__step" data-state={step.failed ? 'failed' : step.done ? 'done' : 'running'} key={at} title={stepWords(step)}>
              <span className="isl-line__node" />
              <span className="isl-line__words">{stepWords(step)}</span>
            </li>
          ))}
        </ol>
        <Checks sessionId={session.sessionId} />
        <SessionCost sessionId={session.sessionId} className="isl-cost" />
        <button className="isl-btn isl-btn--quiet" onClick={() => openPane(session)}>
          {session.paneId ? t('island.detail.openTerminal') : t('island.detail.openChat')}
        </button>
        <Reply session={session} />
      </div>
      <Preview session={session} step={shown} />
    </div>
  )
}

export function Asking({
  question,
  onAnswer,
}: {
  question: Ask
  onAnswer: (verdict: IslandVerdict, chosen?: string[][]) => void
}): React.JSX.Element {
  const asked = question.touch?.kind === 'ask' ? question.touch.questions : []
  if (asked.length > 0) {
    /* One plain choice is answered here; more is its chat's to draw. */
    const one = asked.length === 1 && !asked[0]!.multi ? asked[0]! : null
    return (
      <div className="isl-ask">
        <code className="isl-ask__what">{asked[0]!.question}</code>
        <div className="isl-ask__row">
          {one ? (
            one.options.map((label, n) => (
              <button key={n} className="isl-btn" onClick={() => onAnswer('allow', [[label]])}>
                {label}
              </button>
            ))
          ) : (
            <span className="isl-ask__note">{t('island.ask.answerInChat')}</span>
          )}
          <span className="isl-ask__gap" />
          <button className="isl-btn" onClick={() => onAnswer('deny')}>
            {t('island.ask.deny')}
          </button>
        </div>
      </div>
    )
  }
  return (
    <div className="isl-ask">
      <code className="isl-ask__what">{askedWords(question)}</code>
      <div className="isl-ask__row">
        {question.from === 'terminal' ? (
          <button className="isl-btn isl-btn--quiet" onClick={() => onAnswer('in_terminal')} title={t('island.ask.answerInTerminalTitle')}>
            {t('island.ask.answerInTerminal')}
          </button>
        ) : (
          <span className="isl-ask__note">{t('island.ask.answeredHere')}</span>
        )}
        <span className="isl-ask__gap" />
        <button className="isl-btn" onClick={() => onAnswer('deny')}>
          {t('island.ask.deny')}
        </button>
        {question.keepable && (
          <button className="isl-btn" onClick={() => onAnswer('always')} title={t('island.ask.alwaysTitle')}>
            {t('island.ask.always')}
          </button>
        )}
        <button className="isl-btn isl-btn--go" onClick={() => onAnswer('allow')}>
          {t('island.ask.allow')}
        </button>
      </div>
    </div>
  )
}

/** The head of the open island: a face, what it is about, and the two controls. */
export function Head({
  mood,
  title,
  sub,
  color,
  onBack,
  pinned,
  onPin,
  onFold,
  sound,
  onSound,
  waiting = 0,
}: {
  mood: Mood
  title: string
  sub: string
  color?: string | null
  onBack?: () => void
  /** Questions from other sessions, kept as a badge rather than the view. */
  waiting?: number
  pinned: boolean
  onPin: () => void
  onFold: () => void
  sound: boolean
  onSound: () => void
}): React.JSX.Element {
  return (
    <div className="isl-head">
      {onBack && (
        <button className="isl-icon" onClick={onBack} aria-label={t('island.head.everySession')} title={t('island.head.everySession')}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="m15 18-6-6 6-6" /></svg>
        </button>
      )}
      <Mascot mood={mood} size={40} color={color} looks />
      <span className="isl-head__text">
        <span className="isl-head__title">{title}</span>
        <span className="isl-head__sub">{sub}</span>
      </span>
      {waiting > 0 && onBack && (
        <button className="isl-head__calls" onClick={onBack} title={t('island.head.waitingTitle')}>
          {t('island.head.waiting', { count: waiting })}
        </button>
      )}
      <button className="isl-icon" onClick={onSound} aria-label={sound ? t('island.head.mute') : t('island.head.unmute')} title={sound ? t('island.head.soundsOn') : t('island.head.soundsOff')}>
        {sound ? (
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M11 5 6 9H3v6h3l5 4zM15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13" /></svg>
        ) : (
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M11 5 6 9H3v6h3l5 4zM22 9l-6 6M16 9l6 6" /></svg>
        )}
      </button>
      <button
        className="isl-icon"
        data-on={pinned ? 'true' : undefined}
        onClick={onPin}
        aria-label={pinned ? t('island.head.unpin') : t('island.head.pin')}
        title={pinned ? t('island.head.unpin') : t('island.head.pin')}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 17v5M9 3h6l-1 6 3 3v2H7v-2l3-3z" /></svg>
      </button>
      <button className="isl-icon" onClick={onFold} aria-label={t('island.head.fold')} title={t('island.head.fold')}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="m6 15 6-6 6 6" /></svg>
      </button>
    </div>
  )
}
