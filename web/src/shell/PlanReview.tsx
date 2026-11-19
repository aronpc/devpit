import { useRef, useState } from 'react'

import type { DevflowPlan, PlannedCard } from '../gen/bindings'
import { ask, commands } from './live'
import { abuseSaid, cut, edited, linesOf, moved } from './planEdit'
import { t } from '../i18n'

/*
 * A Devflow plan waiting on its epic: the cards an agent proposed, none of
 * them made yet. The person moves, cuts or edits them here, and the click on
 * Create is the yes that makes them — or an agent's apply after a yes in its
 * conversation, which then creates what was last kept here.
 */
export function PlanReview({
  projectId,
  plan,
  onChanged,
}: {
  projectId: string
  plan: DevflowPlan
  /** The plan was kept, created or dropped: the card and the board read again. */
  onChanged: () => void
}): React.JSX.Element {
  const [cards, setCards] = useState<readonly PlannedCard[]>(plan.cards)
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  /* What the backend holds, so leaving a field untouched keeps nothing. A
     plan proposed again is a new one: the card pane keys this by its cards. */
  const kept = useRef(JSON.stringify(plan.cards))
  /* The last edit on its way: a click on Create leaves a field, and the edit
     that blur keeps has to land before the cards are made from it. */
  const keeping = useRef<Promise<unknown>>(Promise.resolve())

  const titleOf = (key: string): string => cards.find((card) => card.key === key)?.title ?? key
  /* Kept as soon as it changes: an agent's apply creates what the person last saw. */
  const keep = (next: PlannedCard[], reshaped = false): void => {
    setCards(next)
    if (JSON.stringify(next) === kept.current) return
    kept.current = JSON.stringify(next)
    keeping.current = ask(() => commands.devflowPlanKeep(projectId, plan.epicId, next)).then((answer) => {
      setProblem(answer.error)
      /* A move or a cut changes what the tile counts; typing does not. */
      if (!answer.error && reshaped) onChanged()
    })
  }
  const finish = (call: () => Promise<unknown>): void => {
    setBusy(true)
    void keeping.current.then(() => ask(call)).then((answer) => {
      setBusy(false)
      setProblem(answer.error)
      if (!answer.error) onChanged()
    })
  }

  return (
    <section className="dplan" aria-label={t('workspace.planReview.plan')}>
      <h2 className="cardp__h">{t('workspace.planReview.heading', { count: cards.length })}</h2>
      <p className="dplan__note">
        {plan.edited ? t('workspace.planReview.changedHere') : t('workspace.planReview.proposed')} {t('workspace.planReview.noCardYet')}
      </p>
      <ol className="dplan__list">
        {cards.map((card, at) => (
          <li key={card.key} className="dplan__card">
            <div className="dplan__head">
              <span className="dplan__n">{at + 1}</span>
              <input
                className="dplan__t"
                aria-label={t('workspace.planReview.titleOf', { n: at + 1 })}
                value={card.title}
                onChange={(event) => setCards(edited(cards, card.key, { title: event.target.value }))}
                onBlur={() => keep([...cards])}
              />
              {at > 0 && (
                <button className="conv__act" aria-label={t('workspace.planReview.moveUp', { n: at + 1 })} onClick={() => keep(moved(cards, at, -1), true)}>
                  {t('workspace.planReview.up')}
                </button>
              )}
              {at < cards.length - 1 && (
                <button className="conv__act" aria-label={t('workspace.planReview.moveDown', { n: at + 1 })} onClick={() => keep(moved(cards, at, 1), true)}>
                  {t('workspace.planReview.down')}
                </button>
              )}
              {/* The last card goes with the whole plan, below. */}
              {cards.length > 1 && (
                <button className="conv__act" data-danger="true" aria-label={t('workspace.planReview.cutLabel', { n: at + 1 })} onClick={() => keep(cut(cards, card.key), true)}>
                  {t('workspace.planReview.cut')}
                </button>
              )}
            </div>
            {card.dependsOn.length > 0 && (
              <p className="dplan__by">{t('workspace.planReview.blockedBy', { titles: card.dependsOn.map(titleOf).join(', ') })}</p>
            )}
            <textarea
              className="dplan__ta"
              aria-label={t('workspace.planReview.builds', { n: at + 1 })}
              rows={2}
              value={card.build}
              placeholder={t('workspace.planReview.buildPlaceholder')}
              onChange={(event) => setCards(edited(cards, card.key, { build: event.target.value }))}
              onBlur={() => keep([...cards])}
            />
            <CriteriaField
              key={card.criteria.join('\n')}
              at={at}
              criteria={card.criteria}
              onDone={(criteria) => keep(edited(cards, card.key, { criteria }))}
            />
            {(card.boundary || (card.abuse?.length ?? 0) > 0) && <p className="dplan__by">{abuseSaid(card)}</p>}
            {card.verify && <code className="dplan__verify">{card.verify}</code>}
          </li>
        ))}
      </ol>
      <div className="dplan__acts">
        <button className="btn" disabled={busy} onClick={() => finish(() => commands.devflowPlanDrop(projectId, plan.epicId))}>
          {t('workspace.planReview.drop')}
        </button>
        <button className="btn btn--go" disabled={busy} onClick={() => finish(() => commands.devflowPlanCreate(projectId, plan.epicId))}>
          {t('workspace.planReview.create', { count: cards.length })}
        </button>
      </div>
      {problem && <p className="wtb__no">{problem}</p>}
    </section>
  )
}

/* Criteria typed one per line, kept as the field is left. */
function CriteriaField({
  at,
  criteria,
  onDone,
}: {
  at: number
  criteria: readonly string[]
  onDone: (criteria: string[]) => void
}): React.JSX.Element {
  const [text, setText] = useState(criteria.join('\n'))
  return (
    <textarea
      className="dplan__ta"
      aria-label={t('workspace.planReview.criteriaLabel', { n: at + 1 })}
      rows={Math.max(2, criteria.length)}
      value={text}
      placeholder={t('workspace.planReview.criteriaPlaceholder')}
      onChange={(event) => setText(event.target.value)}
      onBlur={() => onDone(linesOf(text))}
    />
  )
}
