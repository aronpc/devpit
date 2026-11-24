import type { ConversationMarks } from '../gen/bindings'
import { t } from '../i18n'
import type { Entry } from './fileMenu'
import { isSettled, snoozeUntil } from './settling'

/*
 * The session row's context menu.
 *
 * Apart from the component for the same reason `fileMenu` is: a menu is data,
 * and a list of labels in the middle of a component is a list nothing can
 * test. The two differ in how they name the row they act on — a file row
 * publishes `data-path`, a session row publishes `data-id` — so this one
 * carries `run`, which is called with that id.
 */

export interface SessionEntry extends Entry {
  readonly run?: (id: string) => void
}

/** What the shell has to hand over for these to act. */
export interface SessionHands {
  focus: (id: string) => void
  close: (id: string) => void
  rename: (id: string) => void
  mark: (id: string, change: Partial<ConversationMarks>) => void
}

/* Offered only on a conversation that has been spoken in: a terminal, or a
   chat with nothing said yet, has nothing to keep them on. */
const marking = ({ mark, close }: SessionHands, marks: ConversationMarks, now: Date): readonly SessionEntry[] => [
  { rule: true },
  { label: t(marks.pinned ? 'workspace.menu.session.unpin' : 'workspace.menu.session.pin'), run: (id) => mark(id, { pinned: !marks.pinned }) },
  isSettled(marks, now.getTime() / 1000)
    ? { label: t('workspace.menu.session.back'), run: (id) => mark(id, { settled: false, snoozedUntil: null }) }
    : { label: t('workspace.menu.session.settle'), run: (id) => mark(id, { settled: true }) },
  { label: t('workspace.menu.session.hour'), run: (id) => mark(id, { snoozedUntil: snoozeUntil('hour', now) }) },
  { label: t('workspace.menu.session.tomorrow'), run: (id) => mark(id, { snoozedUntil: snoozeUntil('tomorrow', now) }) },
  { label: t('workspace.menu.session.unread'), run: (id) => mark(id, { unread: true }) },
  /* Out of every list, and its tab closes; Settings → Archive brings it back. */
  { label: t('workspace.menu.session.archive'), run: (id) => (mark(id, { archived: true }), close(id)) },
]

export const sessionMenu = (hands: SessionHands, marks?: ConversationMarks, now = new Date()): readonly SessionEntry[] => [
  { label: t('workspace.menu.session.open'), key: '↵', run: hands.focus },
  { rule: true },
  { label: t('workspace.menu.session.rename'), shortcut: 'rename', run: hands.rename },
  ...(marks ? marking(hands, marks, now) : []),
  { rule: true },
  { label: t('workspace.menu.session.close'), bad: true, run: hands.close },
]
