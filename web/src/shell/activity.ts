import { createContext, useContext } from 'react'

import type { Message } from '../gen/bindings'
import { t } from '../i18n'
import type { Act } from './acts'
import type { Segment } from './turnFold'

/*
 * How much of the agent's work the thread draws.
 *
 * Picked per conversation, with a default in Settings. Kept per viewer in the
 * webview's storage, like the chat modes: it changes what is drawn, never
 * what the transcript holds.
 */

export type Activity = 'all' | 'failed' | 'none' | 'zen'

export const ACTIVITIES: readonly { readonly id: Activity; readonly label: string; readonly what: string }[] = [
  { id: 'all', get label() { return t('workspace.activity.showAll') }, get what() { return t('workspace.activity.showAllWhat') } },
  { id: 'failed', get label() { return t('workspace.activity.failed') }, get what() { return t('workspace.activity.failedWhat') } },
  { id: 'none', get label() { return t('workspace.activity.none') }, get what() { return t('workspace.activity.noneWhat') } },
  { id: 'zen', get label() { return t('workspace.activity.zen') }, get what() { return t('workspace.activity.zenWhat') } },
]

const KEY = 'devpit.chatActivity'
/* The Settings default, beside the conversations in the same record. */
const DEFAULT = '*'

const isActivity = (value: unknown): value is Activity => ACTIVITIES.some((one) => one.id === value)

function kept(): Record<string, unknown> {
  try {
    const read: unknown = JSON.parse(localStorage.getItem(KEY) ?? '{}')
    return read && typeof read === 'object' ? (read as Record<string, unknown>) : {}
  } catch {
    return {}
  }
}

/* What a conversation opens with: its own pick, else the default, else all. */
export function activityOf(conversationId: string | null): Activity {
  const all = kept()
  const own = conversationId === null ? undefined : all[conversationId]
  if (isActivity(own)) return own
  return isActivity(all[DEFAULT]) ? all[DEFAULT] : 'all'
}

/* Kept for a conversation, or as the default when there is none. */
export function keepActivity(conversationId: string | null, activity: Activity): void {
  const all = kept()
  all[conversationId ?? DEFAULT] = activity
  try {
    localStorage.setItem(KEY, JSON.stringify(all))
  } catch {
    // Storage refused: the pick still holds while the conversation is open.
  }
}

export const labelOf = (activity: Activity): string => ACTIVITIES.find((one) => one.id === activity)!.label

/* The rows a stack of work draws. */
export function shownRows(rows: readonly Act[], activity: Activity): readonly Act[] {
  if (activity === 'all') return rows
  if (activity === 'failed') return rows.filter((row) => row.failed || !row.done)
  return []
}

/* Whether a turn draws a segment. Zen keeps the answer, and in a turn somebody
   steered every reply, since the earlier ones answered the steers. */
export function drawn(segment: Segment, at: number, answer: number, activity: Activity, steered: boolean): boolean {
  if (activity === 'all' || activity === 'failed') return true
  if (segment.kind === 'work') return false
  if (activity === 'none' || segment.kind === 'said') return true
  return at === answer || steered
}

/* The answers somebody steered: a message sent into a running turn sits just
   before its answer, with no turn of its own. */
export function steeredAnswers(all: readonly Message[]): string[] {
  return all.filter((one, at) => one.role === 'assistant' && all[at - 1]?.role === 'user' && all[at - 1]?.turnId === null).map((one) => one.id)
}

/* Find shows everything while it is open, whatever was picked: a match in a
   hidden row would be a match nobody can see. */
export const seen = (picked: Activity, finding: boolean): Activity => (finding ? 'all' : picked)

export const ActivityContext = createContext<Activity>('all')

export const useActivity = (): Activity => useContext(ActivityContext)
