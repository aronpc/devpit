import type { Routine, RoutineKind, Schedule, ScheduleState } from '../gen/bindings'
import { formatDate } from '../i18n/format'

/* What the Schedules pane says about a row, apart from how it draws it. */

export const ROUTINES: ReadonlyArray<readonly [RoutineKind, string]> = [
  ['morning', 'Morning summary'],
  ['ci', 'CI failures'],
  ['prs', 'Pull requests waiting'],
  ['review', 'Cards waiting in review'],
]

export const DAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']

const STATES: Record<ScheduleState, string> = { waiting: 'Waiting', due: 'Went off', done: 'Done', paused: 'Paused' }

export const stateSaid = (state: ScheduleState): string => STATES[state]

/** Whether it repeats, and when: a reminder goes off once. */
export function repeatSaid(routine: Routine | null): string {
  if (!routine) return 'Once'
  const days = [...new Set(routine.days)].sort()
  if (days.length === 0 || days.length === 7) return 'Every day'
  if (days.join() === '1,2,3,4,5') return 'Weekdays'
  if (days.join() === '0,6') return 'Weekends'
  return days.map((day) => DAYS[day]).join(', ')
}

/** The day, the time and the zone: a routine's own zone, else this window's. */
export function whenSaid(at: number | null, zone?: string | null): string {
  if (at === null) return '—'
  const options: Intl.DateTimeFormatOptions = { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit', timeZoneName: 'short' }
  try {
    return formatDate(new Date(at * 1000), zone ? { ...options, timeZone: zone } : options)
  } catch {
    return formatDate(new Date(at * 1000), options)
  }
}

/** The zone this window is in, as the routines keep it. */
export const zoneNow = (): string => Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'

/** Seconds since the epoch as a `datetime-local` value, in this window's zone, and back. */
export function localInput(at: number): string {
  const date = new Date(at * 1000)
  const pad = (n: number): string => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`
}

export const fromInput = (value: string): number | null => {
  const at = new Date(value).getTime()
  return Number.isFinite(at) ? Math.floor(at / 1000) : null
}

/** Where a row lives: its project, and a reminder's card. */
export const whereSaid = (row: Schedule): string => [row.project, row.cardNumber !== null ? `#${row.cardNumber}` : null].filter(Boolean).join(' · ')
