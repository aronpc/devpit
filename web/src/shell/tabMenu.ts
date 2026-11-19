import { t } from '../i18n'
import type { Entry } from './fileMenu'
import type { Sweep, Tab } from './strip'

/*
 * A tab's context menu, as data.
 *
 * Apart from the component for the same reason `fileMenu` is: a test can ask
 * whether every entry does something, which the `a_control_either_works_or_
 * goes` guard cannot — it checks that a handler exists, not that the handler
 * has anything to call.
 *
 * What Orca offers here and this does not: Pin. Nothing pins, and an entry
 * that cannot be wired leaves the screen — AGENTS.md is explicit — so it is
 * absent rather than greyed out.
 */

export interface TabEntry extends Entry {
  readonly run?: (id: string) => void
}

export interface TabHands {
  close: (id: string) => void
  sweep: (id: string, what: Sweep) => void
  join: (from: string, into: string) => void
}

/* `into` is what the tab could be joined with — `joinable` answers it, so a
   tab that cannot join shows no entry rather than one that fails. */
export const tabMenu = (hands: TabHands, into: readonly Tab[] = []): readonly TabEntry[] => [
  /* No key announced: this window has no close binding, and `shortcuts.test`
     is what caught the one written here from memory. A menu that promises a
     key nothing listens for is worse than a menu that promises none. */
  { label: t('workspace.menu.tab.close'), run: hands.close },
  { label: t('workspace.menu.tab.others'), run: (id) => hands.sweep(id, 'others') },
  { rule: true },
  { label: t('workspace.menu.tab.right'), run: (id) => hands.sweep(id, 'right') },
  { label: t('workspace.menu.tab.left'), run: (id) => hands.sweep(id, 'left') },
  { rule: true },
  ...joins(hands, into),
  { label: t('workspace.menu.tab.all'), bad: true, run: (id) => hands.sweep(id, 'all') },
]

const joins = (hands: TabHands, into: readonly Tab[]): TabEntry[] =>
  into.length === 0
    ? []
    : [
        ...into.map((tab) => ({
          label: t('workspace.menu.tab.join', { name: tab.title ?? t('workspace.menu.tab.terminal') }),
          run: (id: string) => hands.join(id, tab.id),
        })),
        { rule: true },
      ]
