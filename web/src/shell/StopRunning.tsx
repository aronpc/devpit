import { useId, useState } from 'react'

import type { Closing } from './useShell'
import { t } from '../i18n'

/*
 * Closing a terminal that is still doing something.
 *
 * Not `Confirm`: that dialog is for things that cannot be undone, and this one
 * has a tick box and two different sentences. The sentences are the point —
 * stopping an agent halfway through a task and stopping a build are not the
 * same loss, and a prompt that words them alike is a prompt people learn to
 * click through without reading.
 *
 * The tick is a decision, not a default. It is off every time the dialog
 * opens, so a habit of pressing Enter cannot silently turn the prompt off for
 * good.
 *
 * And it is not offered at all for unsaved work. The setting it writes says
 * "stop asking about running terminals"; reading it as "stop asking before
 * discarding what I typed" would be a tick box that quietly destroys files.
 */

export function StopRunning({
  closing,
  onCancel,
  onConfirm,
}: {
  closing: Closing
  onCancel: () => void
  onConfirm: (dontAskAgain: boolean) => void
}): React.JSX.Element {
  const box = useId()
  const [dontAskAgain, setDontAskAgain] = useState(false)
  const { kind } = closing.stops
  const agent = kind === 'agent'
  const unsaved = kind === 'unsaved'

  return (
    <div
      className="ask"
      data-open="true"
      onClick={(event) => event.target === event.currentTarget && onCancel()}
    >
      <div className="ask__box" role="dialog" aria-modal="true">
        <h2 className="ask__t">
          {unsaved ? t('workspace.stopRunning.unsavedTitle') : agent ? t('workspace.stopRunning.agentTitle') : t('workspace.stopRunning.commandTitle')}
        </h2>
        <p className="ask__d">
          {unsaved
            ? t('workspace.stopRunning.unsavedBody')
            : agent
              ? t('workspace.stopRunning.agentBody')
              : t('workspace.stopRunning.commandBody')}
        </p>
        <p className="ask__subj">
          {unsaved ? closing.stops.label : `${closing.tab} · ${closing.stops.label}`}
        </p>
        {!unsaved && (
          <label className="ask__opt" htmlFor={box}>
            <input
              id={box}
              type="checkbox"
              checked={dontAskAgain}
              onChange={(event) => setDontAskAgain(event.target.checked)}
            />
            {t('workspace.stopRunning.dontAsk')}
          </label>
        )}
        <div className="ask__row">
          <button className="btn" onClick={onCancel}>
            {t('workspace.stopRunning.cancel')}
          </button>
          <button className="btn btn--danger" autoFocus onClick={() => onConfirm(dontAskAgain)}>
            {unsaved ? t('workspace.stopRunning.discard') : agent ? t('workspace.stopRunning.stopAgent') : t('workspace.stopRunning.stopClose')}
          </button>
        </div>
      </div>
    </div>
  )
}
