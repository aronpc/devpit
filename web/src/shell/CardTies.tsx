import type { CardDetail, CardRef } from '../gen/bindings'
import { t } from '../i18n'
import { closedLabel } from './planEdit'

/*
 * What ties a card to the others: the epic it was planned under, the cards
 * it waits on, and for an epic its cards and how many are done — counted by
 * the lane they are in, whatever the board calls the one for done work.
 */
export function CardTies({ detail }: { detail: CardDetail }): React.JSX.Element | null {
  const blockers = detail.card.blockedBy ?? []
  const children = detail.children ?? []
  const parent = detail.parent ?? null
  const closed = closedLabel(detail.card)
  if (!parent && !closed && blockers.length === 0 && children.length === 0) return null
  const done = children.filter((child) => child.done).length
  return (
    <section className="ties" aria-label={t('card.plan')}>
      {parent && <Named label={t('card.epic')} cards={[parent]} />}
      {closed && <p className="ties__closed">{closed}</p>}
      {blockers.length > 0 && <Named label={t('card.blockedBy')} cards={blockers} />}
      {children.length > 0 && <Named label={t('card.cardsDone', { count: children.length, done })} cards={children} />}
    </section>
  )
}

function Named({ label, cards }: { label: string; cards: readonly CardRef[] }): React.JSX.Element {
  return (
    <div className="ties__group">
      <span className="fld__l">{label}</span>
      <ul className="ties__list">
        {cards.map((card) => (
          <li key={card.id} className="ties__one" data-done={card.done}>
            {card.number != null && <span className="ties__no">#{card.number}</span>}
            {card.title}
          </li>
        ))}
      </ul>
    </div>
  )
}
