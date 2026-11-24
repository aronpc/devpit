import { useState } from 'react'
import { createPortal } from 'react-dom'

import { t } from '../i18n'
import { moveQuestion } from './board'
import { Confirm } from './Confirm'
import { ask, commands } from './live'

/** A lane a card can be moved to, and the position past its last card. */
export interface LaneChoice {
  readonly id: string
  readonly name: string
  readonly end: number
}

/** The question a move asks when a run is still going on the card. On the
 *  body: the board and the card both animate in. */
export function MoveConfirm({
  question,
  onClose,
  onConfirm,
}: {
  question: string
  onClose: () => void
  onConfirm: () => void
}): React.JSX.Element {
  return createPortal(
    <div onPointerDown={(event) => event.stopPropagation()} onKeyDown={(event) => event.stopPropagation()}>
      <Confirm title={t('card.moveAsk')} body={question} danger={t('card.moveAnyway')} onClose={onClose} onConfirm={onConfirm} />
    </div>,
    document.body,
  )
}

/*
 * Which lane the card is in, and a way to move it without closing the card.
 *
 * To the end of the lane it goes to, as a drop at the end of that lane would:
 * the move is the same command the board uses, so a lane that runs a step
 * starts it here too.
 */
export function CardLane({
  projectId,
  cardId,
  columnId,
  lanes,
  onMoved,
}: {
  projectId: string
  cardId: string
  columnId: string
  lanes: readonly LaneChoice[]
  onMoved: () => void
}): React.JSX.Element {
  const [problem, setProblem] = useState<string | null>(null)
  const [asking, setAsking] = useState<{ to: string; question: string } | null>(null)

  const move = async (to: string, confirmed: boolean): Promise<void> => {
    const lane = lanes.find((one) => one.id === to)
    const answer = await ask(() => commands.cardMove(projectId, cardId, to, lane?.end ?? 0, confirmed))
    const question = moveQuestion(answer, confirmed)
    if (question !== null) return setAsking({ to, question })
    setProblem(answer.error)
    if (!answer.error) onMoved()
  }

  return (
    <>
      <label className="cardp__lane">
        <span className="fld__l">{t('card.lane')}</span>
        <select className="cardp__select" aria-label={t('card.lane')} value={columnId} onChange={(event) => void move(event.target.value, false)}>
          {lanes.map((lane) => (
            <option key={lane.id} value={lane.id}>
              {lane.name}
            </option>
          ))}
        </select>
      </label>
      {problem && <p className="wtb__no">{problem}</p>}
      {asking && (
        <MoveConfirm
          question={asking.question}
          onClose={() => setAsking(null)}
          onConfirm={() => {
            setAsking(null)
            void move(asking.to, true)
          }}
        />
      )}
    </>
  )
}
