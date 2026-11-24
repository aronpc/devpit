import { t } from '../i18n'
import type { SessionEntry } from './sessionMenu'

/*
 * A lane's menu, as data, so `wired` can count it.
 *
 * Renaming used to be a click on the name — on the same head that is the drag
 * handle — and deleting was an X that appeared on top of the step picker.
 */

export interface LaneHands {
  rename: () => void
  addCard: () => void
  shift: (by: -1 | 1) => void
  remove: () => void
  /** Says what the lane is for, or null to read it from its name again. */
  role?: (role: string | null) => void
}

/** What a lane can be for, in the words the menu uses. */
export const ROLES: readonly string[] = ['backlog', 'doing', 'check', 'done']

/* What a lane is for is how an agent knows where its card goes, whatever the
   lane is called. Read from the name until somebody says; the one in force
   is marked, and picking the one somebody chose goes back to the name. */
const roleEntries = (hands: LaneHands, now: { role: string | null; chosen: boolean }): SessionEntry[] =>
  hands.role
    ? [
        { rule: true },
        ...ROLES.map((role) => ({
          label: `${now.role === role ? '✓ ' : ''}${t(`lane.role.${role}`)}${now.role === role && !now.chosen ? ` · ${t('lane.byItsName')}` : ''}`,
          run: () => hands.role?.(now.role === role && now.chosen ? null : role),
        })),
      ]
    : []

export const laneMenu = (
  hands: LaneHands,
  now: { role: string | null; chosen: boolean } = { role: null, chosen: false },
): readonly SessionEntry[] => [
  { label: t('lane.menu.rename'), run: hands.rename },
  { label: t('lane.menu.addCard'), run: hands.addCard },
  { rule: true },
  { label: t('lane.menu.moveLeft'), run: () => hands.shift(-1) },
  { label: t('lane.menu.moveRight'), run: () => hands.shift(1) },
  ...roleEntries(hands, now),
  { rule: true },
  { label: t('lane.menu.delete'), bad: true, run: hands.remove },
]
