import { useEffect, useRef, useState } from 'react'

import { t } from '../i18n'
import { Attachments } from './Attachments'
import { savesBeforeRestart } from './beforeRestart'
import { CardCriteria } from './CardCriteria'
import { CardDescription } from './CardDescription'
import { CardDevflow } from './CardDevflow'
import { CardDiff } from './CardDiff'
import { CardElsewhere } from './CardElsewhere'
import { CardLane, type LaneChoice } from './CardLane'
import { CardPlay } from './CardPlay'
import { CardSessionHistory } from './CardSessionHistory'
import { CardSessions } from './CardSessions'
import { CardTies } from './CardTies'
import { LessonProposals } from './LessonProposals'
import { PlanReview } from './PlanReview'
import { liveWork, stopLiveWork } from './liveWork'
import { CardWork } from './CardWork'
import { Comments } from './Comments'
import { CardEnding, CardHeader, type Ending } from './CardHeader'
import { dueLabel, fromField, nearness, toField, toTimeField } from './due'
import { useCard } from './useCard'
import { useShell } from './useShell'
import { abandoned, committed } from './typing'
import { money } from './chat'

/*
 * One card, open over the board.
 *
 * The screen the board never had. Clicking a tile did nothing — `role="button"`
 * with no `onClick` — so `body` was a column the backend could write and
 * nothing could show, and a deadline, a conversation and a file had nowhere to
 * live at all.
 *
 * Over the board rather than beside it: the board is a layout and this is one
 * thing, and a panel that squeezes six lanes into four to show you one card
 * makes you lose your place to read it.
 */

/** What Escape does in an open card.
 *
 *  A dialog or a menu over the card hears it first; then the field being typed
 *  in, which only lets go of the keyboard; only then the card. Closing on the
 *  first Esc took a half-written description with it. */
export function escapeMeans(
  focused: Element | null,
  confirming: boolean,
  menu = false,
): 'nothing' | 'blur' | 'close' {
  if (confirming || menu) return 'nothing'
  if (focused?.closest('input, textarea, select, [contenteditable="true"]')) return 'blur'
  return 'close'
}

export function CardPane({
  cardId,
  onClose,
  onChanged,
  onArchived,
  lanes,
}: {
  cardId: string
  onClose: () => void
  onChanged: () => void
  /** The board's lanes, for moving the card without closing it. */
  lanes?: readonly LaneChoice[]
  /** Told after an archive, so the board can offer to take it back. */
  onArchived?: (cardId: string) => void
}): React.JSX.Element {
  const { project, closeNow } = useShell()
  const card = useCard(project?.id ?? null, cardId, onChanged)
  const detail = card.detail

  const [title, setTitle] = useState('')
  const [body, setBody] = useState('')
  const [dirty, setDirty] = useState(false)
  /* Said where the Save button was: the fields save as they lose focus. */
  const [saved, setSaved] = useState(false)
  const [ending, setEnding] = useState<Ending | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  /* Counted, not flagged: a write that lands after more was typed must not
     call the newer words saved. */
  const edits = useRef(0)
  const edited = (): void => {
    edits.current += 1
    setDirty(true)
    setSaved(false)
  }

  /* The fields follow the card until they are touched. After that they are
     what was typed: a reload landing mid-sentence must not take the sentence. */
  useEffect(() => {
    if (!detail || dirty) return
    setTitle(detail.card.title)
    setBody(detail.card.body)
  }, [detail, dirty])

  /* Hidden with the board or swapped for another card, the pane goes without
     a blur, so what was typed is written on the way out. */
  const leaving = useRef<(() => Promise<unknown>) | null>(null)
  leaving.current = dirty ? () => card.save(title.trim() || 'Untitled', body) : null
  useEffect(() => () => void leaving.current?.(), [])
  /* Nor is an update's restart a blur: the window is asked to put down what it
     holds first, and a half-typed description is exactly that. */
  useEffect(() => savesBeforeRestart(() => leaving.current?.()), [])

  /* What was typed stays typed until the write is kept: a refusal said next
     to an emptied field has already lost the words. */
  const save = async (): Promise<string | null> => {
    if (!dirty) return null
    const at = edits.current
    const refused = await card.save(title.trim() || 'Untitled', body)
    if (refused || edits.current !== at) return refused
    leaving.current = null
    setDirty(false)
    setSaved(true)
    return null
  }

  /* The fields save on blur, and a close by Esc or by a click outside is not
     a blur — so closing is the last chance to keep what was typed. A refused
     write keeps the card open, where the refusal is said. */
  const close = (): void => {
    if (!dirty) return onClose()
    void save().then((refused) => refused === null && onClose())
  }

  useEffect(() => {
    const key = (event: KeyboardEvent): void => {
      if (!abandoned(event)) return
      const focused = document.activeElement
      const means = escapeMeans(
        focused,
        document.querySelector('.ask') !== null,
        /* Not the sidebar's menus, which stay in the page while hidden. */
        document.querySelector('[role="menu"]:not([hidden])') !== null,
      )
      if (means === 'blur') (focused as HTMLElement).blur()
      if (means === 'close') close()
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  })

  /* A press that starts in the box and ends outside it — selecting text past
     the edge — is not a click on the backdrop, though the browser says it is. */
  const pressedOutside = useRef(false)

  const near = nearness(detail?.card.dueAt ?? null)

  return (
    <div
      className="cardp"
      data-open="true"
      onPointerDown={(event) => {
        pressedOutside.current = event.target === event.currentTarget
      }}
      onClick={(event) => pressedOutside.current && event.target === event.currentTarget && close()}
    >
      <div className="cardp__box" role="dialog" aria-modal="true" aria-label={t('card.card')}>
        <CardHeader
          column={detail?.columnName}
          problem={problem}
          onArchive={() => setEnding({ what: 'archive' })}
          onDelete={() => setEnding({ what: 'delete' })}
          onClose={close}
        />

        {!detail && !card.error && <p className="pref__d">{t('card.opening')}</p>}
        {card.error && <p className="wtb__no">{card.error}</p>}

        {detail && (
          <div className="cardp__in cardp__grid">
            <div className="cardp__main">
              {detail.card.number != null && <span className="cardp__no">#{detail.card.number}</span>}
              <input
                className="cardp__title"
                value={title}
                aria-label={t('card.title')}
                onChange={(event) => {
                  setTitle(event.target.value)
                  edited()
                }}
                onBlur={() => void save()}
                onKeyDown={(event) => committed(event) && event.currentTarget.blur()}
              />

              <h2 className="cardp__h">{t('card.description')}</h2>
              <CardDescription
                body={body}
                onChange={(next) => {
                  setBody(next)
                  edited()
                }}
                onDone={() => void save()}
              />
              {saved && !dirty && (
                <p className="cardp__saved" role="status">
                  {t('card.saved')}
                </p>
              )}

              {/* What was typed is kept first: a tick rewrites the body. */}
              <CardCriteria
                criteria={detail.card.criteria ?? []}
                unread={detail.card.criteriaUnread}
                onTick={(at, done) =>
                  void save().then((refused) => {
                    if (refused === null) void card.tick(at, done)
                  })
                }
              />
              {project && detail.card.number != null && (
                <LessonProposals projectId={project.id} source={`#${detail.card.number}`} />
              )}
              {project && detail.plan && (
                <PlanReview
                  key={JSON.stringify(detail.plan.cards)}
                  projectId={project.id}
                  plan={detail.plan}
                  onChanged={() => {
                    card.reload()
                    onChanged()
                  }}
                />
              )}

              {/* A diff only once the card has a checkout; without one, still
                  its review's findings, which are set aside nowhere else. */}
              <CardDiff key={cardId} cardId={cardId} checkout={detail.worktree?.exists === true} />
              <CardElsewhere cardId={cardId} />

              <Comments
                comments={detail.comments}
                onSay={card.comment}
                onEdit={card.editComment}
                onDelete={card.deleteComment}
              />
            </div>

            <div className="cardp__side">
              {project && lanes && lanes.length > 0 && (
                <CardLane
                  projectId={project.id}
                  cardId={cardId}
                  columnId={detail.card.columnId}
                  lanes={lanes}
                  onMoved={() => {
                    card.reload()
                    onChanged()
                  }}
                />
              )}
              <CardPlay cardId={cardId} step={detail.columnStep} onPlayed={card.reload} />

              <div className="cardp__row">
                <label className="cardp__due" data-near={near ?? 'none'}>
                  <span className="fld__l">{t('card.due')}</span>
                  <input
                    type="date"
                    className="cardp__date"
                    value={toField(detail.card.dueAt)}
                    onChange={(event) => {
                      const time = toTimeField(detail.card.dueAt, detail.card.dueTime)
                      card.setDue(fromField(event.target.value, time), time !== '')
                    }}
                  />
                  {/* A time is optional: a day alone reminds that morning. */}
                  {detail.card.dueAt !== null && (
                    <input
                      type="time"
                      className="cardp__date"
                      aria-label={t('card.dueAt')}
                      value={toTimeField(detail.card.dueAt, detail.card.dueTime)}
                      onChange={(event) =>
                        card.setDue(fromField(toField(detail.card.dueAt), event.target.value), event.target.value !== '')
                      }
                    />
                  )}
                  {near && <span className="cardp__near">{dueLabel(detail.card.dueAt, undefined, detail.card.dueTime)}</span>}
                </label>
                {detail.card.dueAt !== null && (
                  <button className="conv__act" onClick={() => card.setDue(null)}>
                    {t('card.clear')}
                  </button>
                )}
                {money(detail.card.costUsd ?? 0) && (
                  <span className="cardp__cost">{t('card.spent', { amount: money(detail.card.costUsd ?? 0) ?? '' })}</span>
                )}
              </div>

              <CardTies detail={detail} />

              <CardDevflow cardId={cardId} />

              <CardWork
                cardId={cardId}
                title={detail.card.title}
                worktree={detail.worktree}
                runs={detail.runs}
                onChanged={card.reload}
              />

              <CardSessions cardId={cardId} title={detail.card.title} sessions={detail.sessions} onChanged={card.reload} />

              <CardSessionHistory cardId={cardId} running={detail.sessions.length} />

              <Attachments pinned={detail.pinned} onPin={card.pin} onUnpin={card.unpin} />
            </div>
          </div>
        )}
      </div>

      {/* Beside the box, not inside it: the box animates in, and a transform
          on an ancestor would anchor the dialog to the box. */}
      {ending && (
        <CardEnding
          ending={ending}
          detail={detail}
          archive={card.archive}
          remove={card.remove}
          live={liveWork(detail?.sessions ?? [])}
          stopLive={() =>
            project
              ? stopLiveWork(project.id, cardId, liveWork(detail?.sessions ?? []), closeNow)
              : Promise.resolve(t('card.noProject'))
          }
          onAsk={setEnding}
          onProblem={setProblem}
          onDone={(what) => {
            if (what === 'archive') onArchived?.(cardId)
            onClose()
          }}
        />
      )}
    </div>
  )
}
