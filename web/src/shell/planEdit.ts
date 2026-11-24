import type { Card, CardRef, PlannedCard } from '../gen/bindings'
import { t } from '../i18n'

/*
 * A Devflow plan as the person changes it on the board, before any card
 * exists: moved, cut or edited. Functions so the tests call them — a cut
 * that left a card waiting on the one cut is a plan the backend refuses.
 */

/** The card at `at` moved `by` places, within the plan. */
export function moved(cards: readonly PlannedCard[], at: number, by: number): PlannedCard[] {
  const to = at + by
  if (to < 0 || to >= cards.length) return [...cards]
  const next = [...cards]
  const [card] = next.splice(at, 1)
  next.splice(to, 0, card!)
  return next
}

/** The plan without one card, and nothing left waiting on it. */
export function cut(cards: readonly PlannedCard[], key: string): PlannedCard[] {
  return cards
    .filter((card) => card.key !== key)
    .map((card) => ({ ...card, dependsOn: card.dependsOn.filter((on) => on !== key) }))
}

/** One card changed, the rest as they were. */
export function edited(cards: readonly PlannedCard[], key: string, change: Partial<PlannedCard>): PlannedCard[] {
  return cards.map((card) => (card.key === key ? { ...card, ...change, key: card.key } : card))
}

/** A card's trust boundary and abuse cases, which become its `Abuse case:` criteria. */
export function abuseSaid(card: Pick<PlannedCard, 'abuse' | 'boundary'>): string {
  const cases = card.abuse ?? []
  const said = cases.length > 0 ? t('workspace.plan.abuseCases', { cases: cases.join('; ') }) : t('workspace.plan.noAbuse')
  return card.boundary ? t('workspace.plan.crossesBoundary', { said }) : said
}

/** Criteria typed one per line. */
export function linesOf(text: string): string[] {
  return text
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '')
}

/** What still blocks a card: the ones it waits on that are neither done nor released. */
export function stillBlocking(blockedBy: readonly CardRef[] | undefined): CardRef[] {
  return (blockedBy ?? []).filter((by) => !by.done && !by.released)
}

/** How a card left the plan without its own evidence, or null while it is in it. */
export function closedLabel(card: Pick<Card, 'cut' | 'supersededBy'>): string | null {
  if (card.supersededBy) return t('workspace.plan.mergedInto', { card: named(card.supersededBy) })
  return card.cut ? t('workspace.plan.cutFromPlan') : null
}

/** Another card as a person says it: `#42 title`, or the title alone before it had a number. */
export function named(card: CardRef): string {
  return card.number != null ? `#${card.number} ${card.title}` : card.title
}
