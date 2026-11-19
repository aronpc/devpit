import { useEffect, useState } from 'react'

import type { Heartbeat } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'

/*
 * Settings → Assistant: how often an orchestrator's heartbeat looks while its
 * Assistant mode is on, and what counts as needing the person. The mode
 * itself is a toggle in each orchestrator's chat.
 */

const EVERY = [5, 10, 15, 30, 60]
const WAITED = [5, 10, 15, 30, 60]

const TRIGGERS: ReadonlyArray<readonly ['ci' | 'waiting' | 'blocked', string]> = [
  ['ci', 'workspace.assistant.triggerCi'],
  ['waiting', 'workspace.assistant.triggerWaiting'],
  ['blocked', 'workspace.assistant.triggerBlocked'],
]

export function AssistantSettings(): React.JSX.Element {
  const [heartbeat, setHeartbeat] = useState<Heartbeat | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    void ask(() => commands.assistantHeartbeat()).then((said) => setHeartbeat(said.data))
  }, [])

  const save = (next: Heartbeat): void => {
    setHeartbeat(next)
    void ask(() => commands.assistantHeartbeatSet(next)).then((said) => {
      if (said.data) setHeartbeat(said.data)
      setProblem(said.error)
    })
  }

  if (!heartbeat) return <p className="acc__note">{t('workspace.assistant.reading')}</p>
  return (
    <>
      <p className="acc__note">
        {t('workspace.assistant.note')}
      </p>
      {problem && <p className="acc__note voice__bad" role="alert">{problem}</p>}
      <div className="pref">
        <span className="pref__body">
          <span className="pref__t">{t('workspace.assistant.lookEvery')}</span>
        </span>
        <select aria-label={t('workspace.assistant.every')} value={heartbeat.minutes} onChange={(change) => save({ ...heartbeat, minutes: Number(change.target.value) })}>
          {EVERY.map((minutes) => <option key={minutes} value={minutes}>{t('workspace.assistant.min', { minutes })}</option>)}
        </select>
      </div>
      <div className="acc__sub">{t('workspace.assistant.needsYou')}</div>
      {TRIGGERS.map(([key, label]) => (
        <div className="pref" key={key}>
          <label className="pref__body voice__row">
            <input type="checkbox" checked={heartbeat[key]} onChange={(change) => save({ ...heartbeat, [key]: change.target.checked })} />
            {t(label)}
          </label>
          {key === 'waiting' && (
            <select aria-label={t('workspace.assistant.stoppedLabel')} value={heartbeat.waitingMinutes} disabled={!heartbeat.waiting} onChange={(change) => save({ ...heartbeat, waitingMinutes: Number(change.target.value) })}>
              {WAITED.map((minutes) => <option key={minutes} value={minutes}>{t('workspace.assistant.moreThan', { minutes })}</option>)}
            </select>
          )}
        </div>
      ))}
    </>
  )
}
