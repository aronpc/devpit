import { useCallback, useMemo, useState } from 'react'

import type { PaneRunning } from '../gen/bindings'
import { ask, commands } from './live'
import { inOrder } from './inOrder'
import { stopsOnClose, type Stops } from './running'
import type { Tab } from './strip'

/*
 * Closing a terminal that is still doing something.
 *
 * Between the cross and the strip, because the question "is anything running
 * in this tab" is one the strip cannot answer: it draws tabs, and what is
 * running belongs to panes. Closing a terminal tab kills every tmux window
 * under it, so a stray click on a cross is the difference between an agent
 * finishing its task and an agent that never comes back.
 *
 * The prompt can be turned off, and only by ticking a box — never by a
 * default, and never by pressing Enter twice in a row. See `StopRunning`.
 */

/** A close that is waiting to be confirmed, and what it would stop. */
export interface Closing {
  readonly id: string
  /** What the tab is called, so the prompt can name what is being closed. */
  readonly tab: string
  readonly stops: Stops
}

export interface Guard {
  /** Closes, or stops to ask when there is something to lose. */
  close: (id: string) => void
  readonly closing: Closing | null
  confirmClose: (dontAskAgain: boolean) => void
  cancelClose: () => void
  /** Answers the settings read, where `null` means never asked — which asks. */
  setConfirmStop: (asked: boolean | null) => void
}

export function useClosing({
  open,
  closeNow,
  running,
  unsaved,
}: {
  open: readonly Tab[]
  /** What closing actually does, once it has been decided. */
  closeNow: (id: string) => void
  running: readonly PaneRunning[]
  /** The tabs holding an edit that is not on disk. */
  unsaved: ReadonlySet<string>
}): Guard {
  const [closing, setClosing] = useState<Closing | null>(null)
  /* Until the settings are read, asking is the safe answer: an unnecessary
     prompt costs a click, and a missing one costs the work in the terminal. */
  const [confirmStop, setStop] = useState(true)

  const close = useCallback(
    (id: string) => {
      const tab = open.find((one) => one.id === id)
      /* Unsaved work is asked about whether or not the prompt is switched
         off. The setting says "stop asking about running terminals", and
         reading it as "stop asking before discarding what I typed" would be
         a setting that quietly destroys work. */
      const stops: Stops | null = unsaved.has(id)
        ? { kind: 'unsaved', label: tab?.title ?? 'this file' }
        : confirmStop
          ? stopsOnClose(running, tab)
          : null
      if (!stops) return closeNow(id)
      setClosing({ id, tab: tab?.title ?? 'this terminal', stops })
    },
    [open, closeNow, running, confirmStop, unsaved],
  )

  const confirmClose = useCallback(
    (dontAskAgain: boolean) => {
      setClosing((waiting) => {
        if (waiting) closeNow(waiting.id)
        return null
      })
      if (!dontAskAgain) return
      setStop(false)
      void inOrder('settings', () => ask(() => commands.settingsWrite(null, null, null, false, null, null, null, null)))
    },
    [closeNow],
  )

  const cancelClose = useCallback(() => setClosing(null), [])
  const setConfirmStop = useCallback((asked: boolean | null) => setStop(asked ?? true), [])

  return useMemo(
    () => ({ close, closing, confirmClose, cancelClose, setConfirmStop }),
    [close, closing, confirmClose, cancelClose, setConfirmStop],
  )
}
