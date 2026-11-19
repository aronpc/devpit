import { currentLocale } from './index'

/*
 * The Intl readers a screen calls for a number, a date or a span of time, in
 * the locale the core resolved and the LocaleRoot carries — never the
 * machine's, because a separator or a month's name is something a person
 * reads without looking, and where the code happens to run is no way to
 * choose one. A caller with its own words to say names a locale out loud.
 */

export function formatDate(at: Date | number, options: Intl.DateTimeFormatOptions = {}, locale = currentLocale()): string {
  return new Intl.DateTimeFormat(locale, options).format(at)
}

export function formatNumber(value: number, options: Intl.NumberFormatOptions = {}, locale = currentLocale()): string {
  return new Intl.NumberFormat(locale, options).format(value)
}

export function formatRelative(
  value: number,
  unit: Intl.RelativeTimeFormatUnit,
  options: Intl.RelativeTimeFormatOptions = {},
  locale = currentLocale(),
): string {
  return new Intl.RelativeTimeFormat(locale, options).format(value, unit)
}
