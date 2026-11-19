import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { FocusSummary } from './FocusSummary'
import { minutesIn } from './headsDown'
import { useKeys, useShortcut } from './useShortcut'
import { ask, commands } from './live'
import { stamp, useHeadsDown } from './useHeadsDown'

/*
 * Going into a focus, and what it says while it is on.
 *
 * The key is listened for here rather than in `AppShell`, where the window's
 * other keys live, for the reason this repository already learned the hard
 * way: ⌘P was printed on a button no listener had heard of. The control that
 * announces a key is the control that hears it.
 *
 * Nothing is hidden while a focus is on. The project on screen stays on
 * screen; what changes is that a notice from somewhere else stops calling and
 * waits to be shown on the way out.
 */

/** How often the minutes on the pill are redrawn. Minutes are what it says,
 *  so a finer tick would re-render for a sentence that did not change. */
const A_TICK = 30_000

export function HeadsDown({
  projectId,
  waiting = 0,
  onOpenCard,
}: {
  projectId: string | null
  /** How many notices the door is holding. Shown, never acted on here. */
  waiting?: number
  /** Where a held notice sends you, when the summary is acted on. */
  onOpenCard?: (cardId: string) => void
}): React.JSX.Element | null {
  const { focus, enter, leave } = useHeadsDown()
  /* The focus that just ended, kept only long enough to say what it held. */
  const [ended, setEnded] = useState<typeof focus>(null)
  /* The length to give the next focus. Zero is none, which is the default:
     a timebox is offered, never imposed. */
  const [minutes, setMinutes] = useState(0)
  /* Off unless somebody turned it on in Settings. Unfinished, and an
     unfinished thing does not get to be in everybody's top bar. */
  const [offered, setOffered] = useState(false)

  useEffect(() => {
    void ask(() => commands.settingsRead()).then((answer) => {
      setOffered(answer.data?.focusMode === true)
    })
  }, [])
  const [now, setNow] = useState(() => Date.now() / 1000)

  const on = focus !== null

  useEffect(() => {
    if (!on) return undefined
    setNow(Date.now() / 1000)
    const tick = window.setInterval(() => setNow(Date.now() / 1000), A_TICK)
    return () => window.clearInterval(tick)
  }, [on])

  /* Stamped on the root element, the way the theme is: the shell reads it in
     CSS, and `AppShell` does not have to hold this state to pass it down. */
  useEffect(() => {
    const root = document.documentElement
    const written = stamp(focus, now)
    if (written) root.dataset.headsDown = written
    else delete root.dataset.headsDown
    return () => {
      delete root.dataset.headsDown
    }
  }, [focus, now])

  /* Opening another project ends the focus, without asking. A focus is on one
     project by definition, and a question here would be asking whether the
     person meant the thing they just did. The summary follows, the same as
     leaving by the button. */
  useEffect(() => {
    if (!focus || !projectId) return
    if (focus.projectId === projectId) return
    setEnded(focus)
    leave()
  }, [focus, projectId, leave])

  const toggle = (): void => {
    if (focus) {
      setEnded(focus)
      leave()
    } else if (projectId) enter(projectId, minutes)
  }

  /* One meaning, and only one. Peeking used to share this click once
     something was held, which left the button with no way out of the focus at
     exactly the moment somebody would want one — a control with two invisible
     meanings, which this window already learned not to ship. What is held is
     in the bell beside it, under its own heading. */

  useShortcut('focus', () => {
    // Off in Settings means off for the key too; a focus already on can
    // still be left.
    if (!focus && (!offered || !projectId)) return false
    toggle()
  })
  const keys = useKeys()('focus')

  // Only with a project open: a focus is on one, and there is nothing to be in
  // the middle of before that. And only when it is turned on.
  if (!projectId || !offered) return null

  return (
    <>
      {ended && (
        <FocusSummary
          ended={ended}
          onOpenCard={(cardId) => {
            setEnded(null)
            onOpenCard?.(cardId)
          }}
          onClose={() => setEnded(null)}
        />
      )}
      {!focus && (
        /* Offered, never imposed: none is the default, and the evidence for a
           fixed break is contradictory and only about students. Whoever likes
           a pomodoro picks 25. */
        <select
          className="hdown__box"
          aria-label={t('workspace.focus.length')}
          value={String(minutes)}
          onChange={(event) => setMinutes(Number(event.target.value))}
        >
          <option value="0">{t('workspace.focus.noEnd')}</option>
          <option value="25">{t('workspace.focus.minutes', { count: 25 })}</option>
          <option value="50">{t('workspace.focus.minutes', { count: 50 })}</option>
          <option value="90">{t('workspace.focus.minutes', { count: 90 })}</option>
        </select>
      )}
      <button
      className="hdown"
      data-on={String(on)}
      onClick={toggle}
      title={t('workspace.focus.focusKeys', { keys })}
      aria-pressed={on}
    >
      {focus
        ? t(waiting > 0 ? 'workspace.focus.runningOutside' : 'workspace.focus.running', { minutes: minutesIn(focus, now), waiting })
        : t('workspace.focus.focus')}
      </button>
    </>
  )
}
