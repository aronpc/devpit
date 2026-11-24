import type { Criterion, UnreadCriteria } from '../gen/bindings'
import { t } from '../i18n'

/*
 * A card's acceptance criteria, ticked where they live: the `## Acceptance
 * criteria` list in its body. A tick is an edit of that line, so the card
 * reads the same in the description, to an agent, and anywhere it is pasted.
 */
export function CardCriteria({
  criteria,
  unread,
  onTick,
}: {
  criteria: readonly Criterion[]
  unread?: UnreadCriteria | null
  onTick: (at: number, done: boolean) => void
}): React.JSX.Element | null {
  if (criteria.length === 0) return unread ? <Unread unread={unread} /> : null
  const done = criteria.filter((one) => one.done).length
  return (
    <section aria-label={t('card.criteria')}>
      <h2 className="cardp__h">
        {t('card.criteriaCount', { done, count: criteria.length })}
      </h2>
      <ul className="crit">
        {criteria.map((one, at) => (
          <li key={at}>
            <label className="crit__one" data-done={one.done}>
              <input type="checkbox" checked={one.done} onChange={(event) => onTick(at, event.target.checked)} />
              <span>{one.text}</span>
            </label>
          </li>
        ))}
      </ul>
    </section>
  )
}

/* Checkboxes the engine does not read: the gate holds the card until they move. */
function Unread({ unread }: { unread: UnreadCriteria }): React.JSX.Element {
  const said = unread.heading
    ? t('card.unreadUnder', { count: unread.count, heading: unread.heading })
    : t('card.unreadNoHeading', { count: unread.count })
  return (
    <p className="crit__note" role="note">
      {said}
    </p>
  )
}
