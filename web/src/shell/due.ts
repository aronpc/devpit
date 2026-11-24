import { t } from '../i18n'
import { formatDate } from '../i18n/format'

/*
 * A card's deadline, and how near it is.
 *
 * The board is the person's own, not a manager's — so a date that has passed
 * is a colour and a phrase, never a badge shouting at them. Three states is
 * all the screen needs, and anything further out than a week is just a date.
 */

export type Nearness = 'past' | 'today' | 'soon' | 'later'

const DAY = 24 * 3600

/** Local midnight of the day `seconds` falls in, in the reader's own zone. */
function dayOf(seconds: number): number {
  const at = new Date(seconds * 1000)
  return new Date(at.getFullYear(), at.getMonth(), at.getDate()).getTime() / 1000
}

/*
 * Compared by day and not by the clock.
 *
 * A card due at 09:00 is not "overdue" at 09:01 — it is due today, and it
 * stays due today until the day ends. Comparing timestamps would turn every
 * deadline red halfway through the morning it was set for.
 */
export function nearness(due: number | null, now = Date.now() / 1000): Nearness | null {
  if (!due) return null
  const days = Math.round((dayOf(due) - dayOf(now)) / DAY)
  if (days < 0) return 'past'
  if (days === 0) return 'today'
  if (days <= 7) return 'soon'
  return 'later'
}

/** The date, in the locale the person chose — the year only when it is not this one,
 *  and the time when one was chosen. */
export function dueLabel(due: number | null, now = Date.now() / 1000, timed = false): string {
  if (!due) return ''
  const at = new Date(due * 1000)
  const near = nearness(due, now)
  const time = timed ? formatDate(at, { hour: '2-digit', minute: '2-digit' }) : ''
  if (near === 'today') return time ? t('workspace.due.todayAt', { time }) : t('workspace.due.today')

  const thisYear = at.getFullYear() === new Date(now * 1000).getFullYear()
  const day = formatDate(at, {
    month: 'short',
    day: 'numeric',
    ...(thisYear ? {} : { year: 'numeric' }),
  })
  return time ? `${day} · ${time}` : day
}

/*
 * What a date field holds, and what it gives back.
 *
 * `<input type="date">` speaks `YYYY-MM-DD` and nothing else, and reading it
 * with `new Date(value)` parses it as UTC — which is the day before, for
 * anyone west of Greenwich. Built from the parts instead, so the date chosen
 * is the date stored.
 */
export function toField(due: number | null): string {
  if (!due) return ''
  const at = new Date(due * 1000)
  const pad = (part: number): string => String(part).padStart(2, '0')
  return `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}`
}

export function fromField(value: string, time = ''): number | null {
  const parts = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value.trim())
  if (!parts) return null
  const [, year, month, day] = parts
  const clock = /^(\d{2}):(\d{2})$/.exec(time.trim())
  /* A day alone is kept at nine in the morning: the hour its reminder goes
     off, and well away from midnight — a date stored at 00:00 local and read
     back through a daylight-saving boundary can land on the previous day. */
  const [hour, minute] = clock ? [Number(clock[1]), Number(clock[2])] : [DAY_REMINDS_AT, 0]
  return new Date(Number(year), Number(month) - 1, Number(day), hour, minute).getTime() / 1000
}

/** The hour a date without a time of its own reminds at, local. */
export const DAY_REMINDS_AT = 9

/** What a time field holds for a date: `HH:MM` when it carries one. */
export function toTimeField(due: number | null, timed: boolean): string {
  if (!due || !timed) return ''
  const at = new Date(due * 1000)
  const pad = (part: number): string => String(part).padStart(2, '0')
  return `${pad(at.getHours())}:${pad(at.getMinutes())}`
}
