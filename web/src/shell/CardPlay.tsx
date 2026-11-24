import { useState } from 'react'

import type { Step } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { Confirm } from './Confirm'
import { ask, commands } from './live'
import { useShell } from './useShell'

/*
 * Play: do this lane's step on this card, now.
 *
 * One meaning, and only one. A lane with no step says so and leaves the
 * terminal to the button below it in the same section — it used to draw a
 * second "Open a terminal" right above the first.
 *
 * What it does NOT ask is most of what Orca's dialog asks. The card already
 * carries the work: its title, its description, its files, its conversation.
 * Project, "run on", and the whole Smart/GitHub/Linear/Jira row exist to
 * *discover* a task from somewhere else, and here the task is the thing you
 * clicked.
 */

export function CardPlay({
  cardId,
  step,
  onPlayed,
}: {
  cardId: string
  /** What the lane the card is in runs, if anything. */
  step: Step | null
  onPlayed: () => void
}): React.JSX.Element {
  const { project } = useShell()
  const [busy, setBusy] = useState(false)
  const [asking, setAsking] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)

  const play = async (confirmed: boolean): Promise<void> => {
    if (!project) return
    setBusy(true)
    const answer = await ask(() => commands.cardPlay(project.id, cardId, confirmed))
    setBusy(false)
    setProblem(answer.error)
    if (!answer.data) return

    /* Asked once, and only for the kind of step that earns it. `card.move`
       uses the same word for the same reason: a deploy is not fired by a
       click somebody might not have meant. */
    if (answer.data.needsConfirming) return setAsking(true)
    setAsking(false)
    onPlayed()
  }

  if (!step) {
    return (
      <div className="play">
        <span className="play__d">{t('card.laneRunsNothing')}</span>
      </div>
    )
  }

  return (
    <div className="play">
      <button className="btn btn--go" disabled={busy} onClick={() => void play(false)}>
        <Triangle />
        {busy ? t('card.starting') : t('lane.runStep', { name: step.name })}
      </button>
      {step.irreversible && <span className="lstep__warn">{t('step.noUndo')}</span>}
      {problem && <span className="play__no">{problem}</span>}

      {asking && (
        <Confirm
          title={t('lane.runStepAsk', { name: step.name })}
          body={<>{tRich('card.playBody', { checkout: <b>{t('card.playCheckout')}</b> })}</>}
          danger={t('lane.runStep', { name: step.name })}
          onClose={() => setAsking(false)}
          onConfirm={() => void play(true)}
        />
      )}
    </div>
  )
}

const Triangle = (): React.JSX.Element => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" stroke="none">
    <path d="M8 5.5v13l11-6.5z" />
  </svg>
)
