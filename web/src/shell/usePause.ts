import { useCallback, useEffect, useState } from 'react'

import type { Paused } from '../gen/bindings'
import { ask, commands } from './live'
import { onCarried } from './window'

const OFF: Paused = { on: false, until: null }

/**
 * Whether devpit is paused, as every window sees it: asked once, heard on
 * every change, and over on its own at its time without anyone asking.
 */
export function usePause(): {
  paused: Paused
  pauseFor: (until: number | null) => void
  resume: () => void
} {
  const [paused, setPaused] = useState<Paused>(OFF)

  useEffect(() => {
    /* A change heard before the first answer is newer, and wins. Outside the
       app, or before it answers, nothing is paused. */
    let heard = false
    const stop = onCarried<Paused>('pause:changed', (now) => {
      heard = true
      setPaused(now)
    })
    void Promise.resolve()
      .then(() => commands.pauseRead())
      .then((now) => !heard && setPaused(now))
      .catch(() => undefined)
    return stop
  }, [])

  /* Over at its time: the app checks lazily, so the window lets itself go. */
  useEffect(() => {
    if (!paused.on || paused.until === null) return
    const left = paused.until * 1000 - Date.now()
    const over = window.setTimeout(() => setPaused(OFF), Math.max(0, left))
    return () => window.clearTimeout(over)
  }, [paused])

  const told = useCallback((answer: { data: Paused | null }) => answer.data && setPaused(answer.data), [])
  return {
    paused,
    /* `null` is until resumed. */
    pauseFor: (until) => void ask(() => commands.pauseSet(until, until === null)).then(told),
    resume: () => void ask(() => commands.pauseSet(null, false)).then(told),
  }
}

/** The moments a pause can be given, from `now` in seconds: null is until
 *  resumed. An id, not words — the button says it in the locale of whoever
 *  reads it (`pause.option.<id>`). */
export function pauseOptions(now: number): { id: string; until: number | null }[] {
  const tomorrow = new Date(now * 1000)
  tomorrow.setDate(tomorrow.getDate() + 1)
  tomorrow.setHours(9, 0, 0, 0)
  return [
    { id: 'for30', until: now + 30 * 60 },
    { id: 'forHour', until: now + 3600 },
    { id: 'untilMorning', until: tomorrow.getTime() / 1000 },
    { id: 'untilResumed', until: null },
  ]
}
