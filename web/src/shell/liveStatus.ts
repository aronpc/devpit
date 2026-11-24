import { useEffect, useState } from 'react'

import type { LiveSession } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { onHappening } from './window'

/*
 * The live sessions of an account, read for every card and row that shows a
 * session's state — however many are on screen — and only while at least one
 * is. Read again shortly after an agent's hook says something, so busy and
 * waiting show within a second; the clock only catches what no hook tells.
 */

const EVERY_MS = 4000
/* Hooks come in bursts while an agent works: one read for each burst. */
const AFTER_HOOK_MS = 300

interface Watch {
  soon: number | null
  sessions: readonly LiveSession[]
  /* What was last told, as text: a read that found the same is told to nobody. */
  said: string
  heard: Set<(sessions: readonly LiveSession[]) => void>
  timer: number | null
  /* Reads still out. A tick waits for them: tmux under load answers late, and
     a read every tick regardless piles up one waiting thread each. */
  out: number
  /* A hook was heard while a read was out: read once more when it ends. */
  again: boolean
}

const watches = new Map<string, Watch>()

/* One subscription for every watch, from the first one on. */
let hearing = false

function hearHooks(): void {
  if (hearing) return
  hearing = true
  onHappening((happening) => {
    if (happening.what !== 'agent' && happening.what !== 'session') return
    for (const [profileId, watch] of watches) {
      if (watch.timer === null || watch.soon !== null) continue
      watch.soon = window.setTimeout(() => {
        watch.soon = null
        read(profileId, watch, false, true)
      }, AFTER_HOOK_MS)
    }
  })
}

function read(profileId: string, watch: Watch, forced = false, hooked = false): void {
  /* Nobody is looking at a hidden window; it is read again on the next tick. */
  if (!forced && typeof document !== 'undefined' && document.hidden) return
  if (!forced && watch.out > 0) {
    watch.again ||= hooked
    return
  }
  watch.out += 1
  void ask(() => commands.orchestratorSessions(profileId)).then((answer) => {
    watch.out -= 1
    if (watch.again && watch.out === 0) {
      watch.again = false
      read(profileId, watch, false, true)
    }
    const sessions = answer.data?.sessions ?? []
    const said = JSON.stringify(sessions)
    if (said === watch.said) return
    watch.said = said
    watch.sessions = sessions
    for (const tell of watch.heard) tell(watch.sessions)
  })
}

/** Reads again now — after an answer, so the next question shows at once. */
export function refreshSessions(profileId: string): void {
  const watch = watches.get(profileId)
  if (watch) read(profileId, watch, true)
}

/* Stopped on the person first, then busy — what is worth looking at — then by name. */
const weight = (one: LiveSession): number => (one.waiting ? 2 : one.status === 'busy' ? 1 : 0)
export const inOrder = (sessions: readonly LiveSession[]): readonly LiveSession[] =>
  [...sessions].sort((a, b) => weight(b) - weight(a) || a.name.localeCompare(b.name))

export function useLiveSessions(profileId: string | null): readonly LiveSession[] {
  const [sessions, setSessions] = useState<readonly LiveSession[]>(() => (profileId ? watches.get(profileId)?.sessions ?? [] : []))
  useEffect(() => {
    if (!profileId) return
    let watch = watches.get(profileId)
    if (!watch) {
      watch = { sessions: [], said: '', heard: new Set(), timer: null, out: 0, soon: null, again: false }
      watches.set(profileId, watch)
    }
    hearHooks()
    const mine = watch
    mine.heard.add(setSessions)
    if (mine.timer === null) {
      read(profileId, mine, true)
      mine.timer = window.setInterval(() => read(profileId, mine), EVERY_MS)
    } else setSessions(mine.sessions)
    return () => {
      mine.heard.delete(setSessions)
      if (mine.heard.size === 0 && mine.timer !== null) {
        window.clearInterval(mine.timer)
        mine.timer = null
        if (mine.soon !== null) window.clearTimeout(mine.soon)
        mine.soon = null
      }
    }
  }, [profileId])
  return sessions
}

/** A session's state as a word: waiting on the person, working, idle, or gone. */
export function stateOf(sessions: readonly LiveSession[], name: string): 'waiting' | 'busy' | 'idle' | 'gone' {
  const found = sessions.find((one) => one.name === name)
  if (!found) return 'gone'
  if (found.waiting) return 'waiting'
  return found.status === 'busy' ? 'busy' : 'idle'
}

/** A session's state as the person reads it. */
export const stateWord = (state: 'waiting' | 'busy' | 'idle' | 'gone'): string => t(`sessions.state.${state}`)
