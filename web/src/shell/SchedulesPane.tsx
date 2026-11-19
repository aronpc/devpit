import { useCallback, useEffect, useState } from 'react'

import type { Routine, RoutineKind, Schedule, ScheduleProposal, Schedules } from '../gen/bindings'
import { clock, minutes, offsetNow } from './channelRules'
import { ask, type Asked, commands } from './live'
import { DAYS, fromInput, localInput, repeatSaid, ROUTINES, stateSaid, whenSaid, whereSaid, zoneNow } from './schedules'
import { useShell } from './useShell'
import { onCarried } from './window'
import { t } from '../i18n'

/*
 * An orchestrator's Schedules: every reminder it reaches and every routine the
 * person set, in one list, with what can be done to each. Only the person
 * schedules — what the orchestrator proposed waits on top for their click.
 */

export function SchedulesPane(): React.JSX.Element {
  const { project, projects } = useShell()
  const hereId = project?.orchestrator ? project.id : null
  const [list, setList] = useState<Schedules | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const [making, setMaking] = useState<'reminder' | 'routine' | null>(null)
  const [linked, setLinked] = useState<readonly string[]>([])

  const took = useCallback((said: Asked<Schedules>) => {
    if (said.data) setList(said.data)
    setProblem(said.error)
  }, [])
  const read = useCallback(() => {
    if (hereId) void ask(() => commands.schedulesList(hereId, zoneNow(), offsetNow())).then(took)
  }, [hereId, took])
  useEffect(read, [read])
  useEffect(() => onCarried<string>('schedules:changed', (id) => id === hereId && read()), [hereId, read])
  useEffect(() => onCarried<null>('reminder:changed', read), [read])
  useEffect(() => {
    if (hereId) void ask(() => commands.orchestratorLinks(hereId)).then((answer) => setLinked(answer.data ?? []))
  }, [hereId])

  if (!hereId) return <p className="acc__note">{t('workspace.schedules.belongs')}</p>
  /* A change to a reminder's card answers the card, not the list: read the list again. */
  const then = <T,>(call: () => Promise<Asked<T>>): void => void call().then((said) => (said.error ? setProblem(said.error) : read()))
  const act = {
    pause: (row: Schedule, pause: boolean) => void ask(() => commands.scheduleRoutinePause(hereId, row.id, pause)).then(took),
    remove: (row: Schedule) =>
      row.kind === 'routine'
        ? void ask(() => commands.scheduleRoutineDelete(hereId, row.id)).then(took)
        : then(() => ask(() => commands.cardSetDue(row.projectId ?? hereId, row.id, null, false))),
    resolve: (row: Schedule) => then(() => ask(() => commands.reminderDone(row.id))),
    retime: (row: Schedule, at: number) => then(() => ask(() => commands.cardSetDue(row.projectId ?? hereId, row.id, at, true))),
    routine: (id: string | null, routine: Routine) => void ask(() => commands.scheduleRoutineSave(hereId, id, routine)).then(took),
    decide: (proposal: ScheduleProposal, add: boolean) => void ask(() => commands.scheduleProposal(hereId, proposal.id, add, zoneNow(), offsetNow())).then(took),
  }
  const places = projects.filter((one) => one.id === hereId || linked.includes(one.id))

  return (
    <div className="scroll">
      <div className="prefs__in sched">
        <h1 className="prefs__h">{t('workspace.schedules.title')}</h1>
        <p className="acc__note">{t('workspace.schedules.intro')}</p>
        {problem && <p className="acc__note voice__bad" role="alert">{problem}</p>}
        {list?.proposed.map((proposal) => <Proposed key={proposal.id} proposal={proposal} onDecide={(add) => act.decide(proposal, add)} />)}
        <div className="sched__new">
          <button className="btn btn--sm" onClick={() => setMaking(making === 'reminder' ? null : 'reminder')} aria-pressed={making === 'reminder'}>{t('workspace.schedules.newReminder')}</button>
          <button className="btn btn--sm" onClick={() => setMaking(making === 'routine' ? null : 'routine')} aria-pressed={making === 'routine'}>{t('workspace.schedules.newRoutine')}</button>
        </div>
        {making === 'routine' && <RoutineForm onSave={(routine) => (act.routine(null, routine), setMaking(null))} onCancel={() => setMaking(null)} />}
        {making === 'reminder' && (
          <ReminderForm
            places={places.map((one) => [one.id, one.name])}
            onSave={(target, title, at) => void ask(() => commands.scheduleRemind(hereId, target, title, at)).then((said) => (took(said), said.data && setMaking(null)))}
            onCancel={() => setMaking(null)}
          />
        )}
        {list && list.schedules.length === 0 && <p className="acc__note">{t('workspace.schedules.none')}</p>}
        {list && list.schedules.length > 0 && (
          <div className="sched__list" role="table" aria-label={t('workspace.schedules.title')}>
            <div className="sched__row sched__row--head" role="row">
              {(['when', 'what', 'where', 'repeats', 'state', 'lastRun'] as const).map((head) => <span key={head} role="columnheader">{t(`workspace.schedules.head.${head}`)}</span>)}
            </div>
            {list.schedules.map((row) => <Row key={row.id} row={row} act={act} />)}
          </div>
        )}
      </div>
    </div>
  )
}

interface Acts {
  pause: (row: Schedule, pause: boolean) => void
  remove: (row: Schedule) => void
  resolve: (row: Schedule) => void
  retime: (row: Schedule, at: number) => void
  routine: (id: string | null, routine: Routine) => void
}

function Row({ row, act }: { row: Schedule; act: Acts }): React.JSX.Element {
  const [editing, setEditing] = useState(false)
  const [at, setAt] = useState(row.at !== null ? localInput(row.at) : '')
  return (
    <>
      <div className="sched__row" role="row" data-state={row.state}>
        <span role="cell">{whenSaid(row.at, row.routine?.zone)}</span>
        <span role="cell" className="sched__what">{row.title}</span>
        <span role="cell">{whereSaid(row)}</span>
        <span role="cell">{repeatSaid(row.routine)}</span>
        <span role="cell" className="sched__state">{stateSaid(row.state)}</span>
        <span role="cell">{row.lastRun !== null ? whenSaid(row.lastRun) : '—'}</span>
        <span role="cell" className="sched__acts">
          {row.kind === 'routine' && <button className="btn btn--sm" onClick={() => act.pause(row, row.state !== 'paused')}>{row.state === 'paused' ? t('workspace.schedules.resume') : t('workspace.schedules.pause')}</button>}
          {row.kind === 'reminder' && row.state !== 'done' && <button className="btn btn--sm" onClick={() => act.resolve(row)}>{t('workspace.schedules.resolve')}</button>}
          <button className="btn btn--sm" onClick={() => setEditing(!editing)} aria-pressed={editing}>{t('workspace.schedules.edit')}</button>
          <button className="btn btn--sm btn--quiet" onClick={() => act.remove(row)} aria-label={t('workspace.schedules.delete.label', { title: row.title })}>{t('workspace.schedules.delete')}</button>
        </span>
      </div>
      {editing && row.routine && <RoutineForm routine={row.routine} onSave={(routine) => (act.routine(row.id, routine), setEditing(false))} onCancel={() => setEditing(false)} />}
      {editing && !row.routine && (
        <div className="sched__form">
          <input type="datetime-local" aria-label={t('workspace.schedules.newTime', { title: row.title })} value={at} onChange={(change) => setAt(change.target.value)} />
          <button className="btn btn--sm btn--go" onClick={() => { const next = fromInput(at); if (next !== null) act.retime(row, next); setEditing(false) }}>{t('workspace.schedules.save')}</button>
          <button className="btn btn--sm btn--quiet" onClick={() => setEditing(false)}>{t('workspace.schedules.cancel')}</button>
        </div>
      )}
    </>
  )
}

function RoutineForm({ routine, onSave, onCancel }: { routine?: Routine; onSave: (routine: Routine) => void; onCancel: () => void }): React.JSX.Element {
  const [kind, setKind] = useState<RoutineKind>(routine?.kind ?? 'morning')
  const [time, setTime] = useState(clock(routine?.minute ?? 9 * 60))
  const [days, setDays] = useState<number[]>(routine?.days ?? [1, 2, 3, 4, 5])
  return (
    <div className="sched__form">
      <select aria-label={t('workspace.schedules.routine.label')} value={kind} onChange={(change) => setKind(change.target.value as RoutineKind)}>
        {ROUTINES.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
      </select>
      <input type="time" aria-label={t('workspace.schedules.routine.at')} value={time} onChange={(change) => setTime(change.target.value)} />
      {DAYS.map((day, at) => (
        <label key={day} className="chday">
          <input type="checkbox" checked={days.includes(at)} onChange={(change) => setDays(change.target.checked ? [...days, at] : days.filter((one) => one !== at))} />
          {day}
        </label>
      ))}
      <button className="btn btn--sm btn--go" onClick={() => onSave({ kind, minute: minutes(time), days, zone: zoneNow(), offsetMinutes: offsetNow() })}>{t('workspace.schedules.routine.save')}</button>
      <button className="btn btn--sm btn--quiet" onClick={onCancel}>{t('workspace.schedules.cancel')}</button>
    </div>
  )
}

function ReminderForm({ places, onSave, onCancel }: { places: ReadonlyArray<readonly [string, string]>; onSave: (target: string, title: string, at: number) => void; onCancel: () => void }): React.JSX.Element {
  const [title, setTitle] = useState('')
  const [target, setTarget] = useState(places[0]?.[0] ?? '')
  const [at, setAt] = useState(() => localInput(Math.floor(Date.now() / 1000) + 3600))
  const when = fromInput(at)
  return (
    <div className="sched__form">
      <input aria-label={t('workspace.schedules.reminder.of')} placeholder={t('workspace.schedules.reminder.placeholder')} value={title} onChange={(change) => setTitle(change.target.value)} />
      <select aria-label={t('workspace.schedules.reminder.on')} value={target} onChange={(change) => setTarget(change.target.value)}>
        {places.map(([id, name]) => <option key={id} value={id}>{name}</option>)}
      </select>
      <input type="datetime-local" aria-label={t('workspace.schedules.reminder.when')} value={at} onChange={(change) => setAt(change.target.value)} />
      <button className="btn btn--sm btn--go" disabled={!title.trim() || when === null || !target} onClick={() => when !== null && onSave(target, title, when)}>{t('workspace.schedules.reminder.save')}</button>
      <button className="btn btn--sm btn--quiet" onClick={onCancel}>{t('workspace.schedules.cancel')}</button>
    </div>
  )
}

function Proposed({ proposal, onDecide }: { proposal: ScheduleProposal; onDecide: (add: boolean) => void }): React.JSX.Element {
  const what = proposal.routine
    ? t('workspace.schedules.proposed.routine', { routine: ROUTINES.find(([kind]) => kind === proposal.routine?.kind)?.[1] ?? '', time: clock(proposal.routine.minute), repeat: repeatSaid(proposal.routine).toLowerCase() })
    : proposal.reminder
      ? t(proposal.reminder.project ? 'workspace.schedules.proposed.reminderOn' : 'workspace.schedules.proposed.reminder', { title: proposal.reminder.title, when: whenSaid(proposal.reminder.at), project: proposal.reminder.project ?? '' })
      : ''
  return (
    <div className="pref" role="group" aria-label={t('workspace.schedules.proposed.label')}>
      <span className="pref__body">
        <span className="pref__t">{t('workspace.schedules.proposed.said', { what })}</span>
        <span className="pref__d">{proposal.why}</span>
      </span>
      <button className="btn btn--sm" onClick={() => onDecide(false)}>{t('workspace.schedules.drop')}</button>
      <button className="btn btn--sm btn--go" onClick={() => onDecide(true)}>{t('workspace.schedules.add')}</button>
    </div>
  )
}
