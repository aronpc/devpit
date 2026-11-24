import { Fragment, useEffect, useState } from 'react'

import type { CleanRun, Maintenance } from '../gen/bindings'
import { t } from '../i18n'
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
  return days === 0 ? t('workspace.maintenance.untilDeleted') : t('workspace.maintenance.days', { count: days })
}

function lastRun(run: CleanRun, nowMs: number): string {
  return t('workspace.maintenance.lastRun', { ago: ago(run.at, nowMs), removed: run.removed, freed: bytes(run.freedBytes ?? 0) ?? '0 B' })
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
      <div className="acc__sub">{t('workspace.maintenance.background')}</div>
      <PrefSwitch
        on={shown?.powerSaver ?? false}
        onFlip={() => shown && write({ powerSaver: !shown.powerSaver })}
        title={t('workspace.maintenance.powerSaver')}
        said={t('workspace.maintenance.powerSaverSaid')}
        more={shown?.onBattery ? t('workspace.maintenance.onBattery') : undefined}
      />

      <div className="acc__sub">{t('workspace.maintenance.logs')}</div>
      <PrefRow title={t('workspace.maintenance.keep')} said={t('workspace.maintenance.keepSaid')}>
        <select aria-label={t('workspace.maintenance.keep')} value={shown?.keepDays ?? 0} disabled={!shown} onChange={(event) => write({ keepDays: Number(event.target.value) })}>
          {KEEP_DAYS.map((days) => (
            <option key={days} value={days}>
              {kept(days)}
            </option>
          ))}
        </select>
      </PrefRow>
      {shown && shown.keepDays > 0 && (
        <PrefRow title={t('workspace.maintenance.cleanNow')} said={last ? lastRun(last, heardAt) : t('workspace.maintenance.notRun', { count: shown.keepDays })}>
          <button className="btn" disabled={busy} onClick={cleanNow}>
            {busy ? t('workspace.maintenance.deleting') : t('workspace.maintenance.deleteNow')}
          </button>
        </PrefRow>
      )}
      {error && <p className="pref__d">{error}</p>}

      <div className="acc__sub">{t('workspace.maintenance.labs')}</div>
      <PrefSwitch
        on={shown?.projectAgent ?? true}
        onFlip={() => shown && write({ projectAgent: !shown.projectAgent })}
        title={t('workspace.maintenance.projectAgent')}
        said={t('workspace.maintenance.projectAgentSaid')}
      />
      {shown?.projectAgent &&
        shown.projectAgentProjects.map((one) => (
          <Fragment key={one.id}>
            <PrefSwitch
              on={one.on}
              onFlip={() => turn(one.id, !one.on)}
              title={t('workspace.maintenance.agentIn', { name: one.name })}
              said={one.on ? t('workspace.maintenance.agentOn') : t('workspace.maintenance.agentOff')}
            />
            {one.on && (
              <PrefSwitch
                on={one.acceptFacts}
                onFlip={() => acceptFacts(one.id, !one.acceptFacts)}
                title={t('workspace.maintenance.facts', { name: one.name })}
                said={t('workspace.maintenance.factsSaid')}
              />
            )}
          </Fragment>
        ))}
      <PrefSwitch
        on={shown?.canvas ?? true}
        onFlip={() => shown && write({ canvas: !shown.canvas })}
        title={t('workspace.maintenance.canvas')}
        said={t('workspace.maintenance.canvasSaid')}
      />
    </>
  )
}
