import { modes } from './chat'

/*
 * The permission mode and effort last picked on each profile, so a new
 * conversation starts where the person left the last one. Per viewer and
 * lost with the webview's storage: a convenience, not a record — the
 * conversation's own head is what a reopened chat goes back to.
 */

const KEY = 'devpit.chatModes'

export interface Modes {
  readonly permission?: string
  readonly effort?: string
}

function all(): Record<string, Modes> {
  try {
    const kept: unknown = JSON.parse(localStorage.getItem(KEY) ?? '{}')
    return kept && typeof kept === 'object' ? (kept as Record<string, Modes>) : {}
  } catch {
    return {}
  }
}

export function remembered(profileId: string): Modes {
  return all()[profileId] ?? {}
}

export function remember(profileId: string, picked: Modes): void {
  const kept = all()
  kept[profileId] = { ...kept[profileId], ...picked }
  try {
    localStorage.setItem(KEY, JSON.stringify(kept))
  } catch {
    // Storage refused: the pick still holds for this conversation.
  }
}

/* A pick made in a conversation is kept under the conversation too: picked
   and not yet sent, it is newer than what the head says the last turn ran with. */
export const conversationKey = (conversationId: string): string => `conversation:${conversationId}`

/** What a conversation opens in, newest first: a pick not yet sent, what it
 *  last ran with, the profile's last pick. A mode this build no longer offers
 *  is dropped, not sent. */
export function reopened(conversationId: string, stored: Modes, profileId: string | null): Modes {
  return opening({ ...stored, ...remembered(conversationKey(conversationId)) }, profileId ? remembered(profileId) : {})
}

/** The first of `stored` and `last` this build offers. */
export function opening(stored: Modes, last: Modes): Modes {
  const permission = [stored.permission, last.permission].find((one) => modes().some((mode) => mode.id === one))
  return { permission, effort: stored.effort ?? last.effort }
}
