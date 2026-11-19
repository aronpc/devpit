import type { Secret } from '../gen/bindings'
import { t } from '../i18n'
import { formatDate } from '../i18n/format'

/** The intervals a secret's rotation is offered at, in days. */
export const ROTATION_DAYS = [30, 90, 180, 365] as const

/** A day picked in a date field as the moment its reminder goes off: nine in the morning, here. */
export function rotationMoment(day: string): number | null {
  const [year, month, date] = day.split('-').map(Number)
  if (!year || !month || !date) return null
  return new Date(year, month - 1, date, 9, 0, 0, 0).getTime() / 1000
}

/** A moment as the day a date field shows, here. */
export function rotationDay(at: number): string {
  const day = new Date(at * 1000)
  const two = (n: number): string => String(n).padStart(2, '0')
  return `${day.getFullYear()}-${two(day.getMonth() + 1)}-${two(day.getDate())}`
}

/** What a secret's row says about its rotation, or nothing when it has none. */
export function rotationSaid(secret: Secret, now: number): string | null {
  if (secret.rotateAt == null) return null
  const date = formatDate(secret.rotateAt * 1000)
  return t(secret.rotateAt <= now ? 'secrets.dueSince' : 'secrets.rotateBy', { date })
}

/** The rotation picker's value: an interval, a date alone, or none. */
export function rotationChoice(secret: Secret): string {
  if (secret.rotateDays) return String(secret.rotateDays)
  return secret.rotateAt == null ? '' : 'date'
}
