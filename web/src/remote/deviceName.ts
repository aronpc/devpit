/*
 * The name a device offers when it pairs, guessed from its browser and
 * editable before Pair. It only labels the row in Settings — the token is what
 * the machine trusts — so a guess is enough, and a generic one is the failure:
 * a Linux laptop and a Chromebook both came out as "A device".
 */

import { t } from '../i18n'

const BROWSERS: readonly [RegExp, string][] = [
  [/Edg\//, 'Edge'],
  [/Firefox\/|FxiOS\//, 'Firefox'],
  [/Chrome\/|CriOS\//, 'Chrome'],
  [/Safari\//, 'Safari'],
]

function systemOf(agent: string, touchPoints: number): string | null {
  if (/iPhone/.test(agent)) return 'iPhone'
  if (/iPad/.test(agent)) return 'iPad'
  /* iPadOS asks for the desktop site, so its agent says Macintosh; a Mac has
     no touch screen. */
  if (/Macintosh/.test(agent) && touchPoints > 1) return 'iPad'
  if (/Android/.test(agent)) return 'Android'
  if (/CrOS/.test(agent)) return 'Chromebook'
  if (/Mac/.test(agent)) return 'Mac'
  if (/Windows/.test(agent)) return 'Windows'
  if (/Linux|X11/.test(agent)) return 'Linux'
  return null
}

/** What this device calls itself, from its user agent: "iPhone", "Linux", or
 *  the browser when the system says nothing. */
export function deviceName(agent: string, touchPoints = 0): string {
  const system = systemOf(agent, touchPoints)
  if (system) return system
  const browser = BROWSERS.find(([pattern]) => pattern.test(agent))
  return browser ? t('remote.device.browser', { name: browser[1] }) : t('remote.device.aBrowser')
}
