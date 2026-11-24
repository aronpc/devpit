import { useCallback, useEffect, useRef, useState } from 'react'

import type { Card, CardSession, Doing } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { afterLooking, afterState, type Unread } from './unread'
import { useShell } from './useShell'

/*
 * What a tile says about the sessions working on its card, and where its dot
 * goes when pressed.
 */

/** The words for a tile's dot, read when drawn so they follow the chosen language. */
export const doingLabel = (doing: Doing, unread: boolean): string =>
  t(doing === 'done' && unread ? 'card.doing.doneUnread' : `card.doing.${doing}`)

/** The session a dot takes somebody to: one waiting on them, in a terminal they can reach. */
export function waitingSession(sessions: readonly CardSession[]): CardSession | null {
  return sessions.find((session) => session.state === 'waiting' && session.tabId) ?? null
}

/** Cards with a finish nobody has looked at. Looking at one is having it open. */
export function useCardUnread(cards: readonly Card[], looking: string | null): Unread {
  const [unread, setUnread] = useState<Unread>(() => new Set())
  const was = useRef<Readonly<Record<string, Doing | null>>>({})
  const front = looking ?? ''

  useEffect(() => {
    const before = was.current
    was.current = Object.fromEntries(cards.map((card) => [card.id, card.activity]))
    setUnread((held) =>
      cards.reduce((next, card) => {
        const state = card.activity
        /* A card first seen is not news: a board opening with a finished card
           on it has nothing to catch up on. */
        if (!(card.id in before) || before[card.id] === state) return next
        if (state === 'done' || state === 'working' || state === 'waiting') {
          return afterState(next, card.id, state, front ? [front] : [])
        }
        return next
      }, held),
    )
  }, [cards, front])

  useEffect(() => {
    setUnread((held) => afterLooking(held, front ? [front] : []))
  }, [front])

  return unread
}

/** Goes where a tile's dot points: the pane waiting on somebody, or else the card itself. */
export function useVisit(
  projectId: string | null,
  sessions: Readonly<Record<string, readonly CardSession[]>>,
  onOpen: (cardId: string) => void,
): (card: Card) => void {
  const { show } = useShell()
  return useCallback(
    (card: Card) => {
      const waiting = waitingSession(sessions[card.id] ?? [])
      const tabId = waiting?.tabId
      if (!projectId || !tabId) return onOpen(card.id)
      show('term', { id: tabId, title: card.title, cardId: card.id })
      const leafId = waiting.leafId
      if (leafId) void ask(() => commands.sessionFocus(projectId, tabId, leafId))
    },
    [projectId, sessions, show, onOpen],
  )
}
