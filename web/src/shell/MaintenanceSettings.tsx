import { Fragment, useEffect, useState } from 'react'

import type { CleanRun, Maintenance } from '../gen/bindings'
import { bytes } from './disk'
import { ask, commands } from './live'
import { PrefRow } from './PrefRow'
import { PrefSwitch } from './PrefSwitch'
import { ago } from './usageFormat'

/*
 * Settings → Maintenance: how hard devpit works in the background, how long
 * its logs and run artifacts stay, and Labs, the switches for what is in beta.
 * Every default is how devpit behaved before the pane existed.
 */

const KEEP_DAYS = [0, 7, 14, 30, 90] as const

interface Change {
  powerSaver?: boolean
  keepDays?: number
  projectAgent?: boolean
  canvas?: boolean
}

function kept(days: number): string {
  return days === 0 ? 'Until deleted' : `${days} days`
}

function lastRun(run: CleanRun, nowMs: number): string {
  return `Last run ${ago(run.at, nowMs)}: removed ${run.removed}, freed ${bytes(run.freedBytes ?? 0) ?? '0 B'}.`
}

export function MaintenanceSettings(): React.JSX.Element {
  const [shown, setShown] = useState<Maintenance | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  /* When the answer came, so "ago" is read from then rather than during render. */
  const [heardAt, setHeardAt] = useState(0)
  const take = (data: Maintenance | null): void => {
    setShown(data)
    setHeardAt(Date.now())
  }

  useEffect(() => {
    void ask(() => commands.maintenanceRead()).then((answer) => take(answer.data ?? null))
  }, [])

  const write = (change: Change): void => {
    void ask(() =>
      commands.maintenanceWrite(change.powerSaver ?? null, change.keepDays ?? null, change.projectAgent ?? null, change.canvas ?? null),
    ).then((answer) => {
      setError(answer.error)
      if (answer.data) take(answer.data)
    })
  }

  const turn = (projectId: string, on: boolean): void => {
    void ask(() => commands.maintenanceProjectAgent(projectId, on)).then((answer) => {
      setError(answer.error)
      if (answer.data) take(answer.data)
    })
  }

  const acceptFacts = (projectId: string, on: boolean): void => {
    void ask(() => commands.maintenanceAcceptFacts(projectId, on)).then((answer) => {
      setError(answer.error)
      if (answer.data) take(answer.data)
    })
  }

  const cleanNow = (): void => {
    setBusy(true)
    void ask(() => commands.maintenanceCleanNow()).then((answer) => {
      setBusy(false)
      setError(answer.error)
      if (answer.data) take(answer.data)
    })
  }

  const last = shown?.last ?? null
  return (
    <>
      <div className="acc__sub">Background activity</div>
      <PrefSwitch
        on={shown?.powerSaver ?? false}
        onFlip={() => shown && write({ powerSaver: !shown.powerSaver })}
        title="Power saver"
        said="Background checks run four times less often while nobody is at the window or this machine runs on battery. Off, devpit is balanced, as it always was."
        more={shown?.onBattery ? 'This machine is on battery now.' : undefined}
      />

      <div className="acc__sub">Logs and run artifacts</div>
      <PrefRow title="Keep logs and run artifacts" said="devpit's logs, and the runs and reviews Devflow keeps for each card. The newest run of each card always stays.">
        <select aria-label="Keep logs and run artifacts" value={shown?.keepDays ?? 0} disabled={!shown} onChange={(event) => write({ keepDays: Number(event.target.value) })}>
          {KEEP_DAYS.map((days) => (
            <option key={days} value={days}>
              {kept(days)}
            </option>
          ))}
        </select>
      </PrefRow>
      {shown && shown.keepDays > 0 && (
        <PrefRow title="Clean up now" said={last ? lastRun(last, heardAt) : `Not run yet. It also runs on its own, for what is older than ${shown.keepDays} days.`}>
          <button className="btn" disabled={busy} onClick={cleanNow}>
            {busy ? 'Deleting…' : 'Delete now'}
          </button>
        </PrefRow>
      )}
      {error && <p className="pref__d">{error}</p>}

      <div className="acc__sub">Labs</div>
      <PrefSwitch
        on={shown?.projectAgent ?? true}
        onFlip={() => shown && write({ projectAgent: !shown.projectAgent })}
        title="Project agent"
        said="Beta. An agent that knows one project. On, it is offered to each project linked to an orchestrator, and reads a project only once you turn it on for that project, below or from its Project agent page. The orchestrator and the project's sessions ask it, and it draws lessons when a card is done."
      />
      {shown?.projectAgent &&
        shown.projectAgentProjects.map((one) => (
          <Fragment key={one.id}>
            <PrefSwitch
              on={one.on}
              onFlip={() => turn(one.id, !one.on)}
              title={`Project agent in ${one.name}`}
              said={one.on ? 'Learns and answers for this project.' : 'Off: no agent learns or answers for this project. On, it learns the project once.'}
            />
            {one.on && (
              <PrefSwitch
                on={one.acceptFacts}
                onFlip={() => acceptFacts(one.id, !one.acceptFacts)}
                title={`Accept facts that rest only on files in ${one.name}`}
                said="A fact saved from Ask the project whose sources are all files of the project enters its memory without waiting, marked as accepted automatically and undone from its page, as the facts its own read finds do. Lessons and decisions always wait for you."
              />
            )}
          </Fragment>
        ))}
      <PrefSwitch
        on={shown?.canvas ?? true}
        onFlip={() => shown && write({ canvas: !shown.canvas })}
        title="Canvas"
        said="Beta. The Canvas plugin, a whiteboard a project can install. Off, it is not offered."
      />
    </>
  )
}
