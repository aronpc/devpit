import { useEffect, useState } from 'react'

import { setLocale, t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
import type { Display, DisplayBackend, Language, Settings as Stored } from '../gen/bindings'
import { ask, commands } from './live'
import { inOrder } from './inOrder'
import { PrefRow } from './PrefRow'
import { onEvent } from './window'

/* Linux only: the display the next start opens on. */

const NAMED: Record<DisplayBackend, string> = { wayland: 'Wayland', x11: 'X11 (XWayland)' }

/* Backend names are proper nouns; what is not a name, translates. */
const CHOICES: { value: DisplayBackend | null; own: string | null }[] = [
  { value: null, own: null },
  { value: 'wayland', own: 'Wayland' },
  { value: 'x11', own: 'X11' },
]

export function DisplaySettings(): React.JSX.Element | null {
  const [display, setDisplay] = useState<Display | null>(null)
  /* Hears the locale, so a choice made anywhere re-says this row's words. */
  useLocale()

  useEffect(() => {
    void ask(() => commands.displayRead()).then((answer) => setDisplay(answer.data ?? null))
  }, [])

  if (!display?.offered) return null
  const pick = (value: DisplayBackend | null): void =>
    void ask(() => commands.displaySet(value)).then((answer) => answer.data && setDisplay(answer.data))

  return (
    <PrefRow
      title={t('display.title')}
      said={`${display.running ? `${t('display.on', { name: NAMED[display.running] })} ` : ''}${t(`display.why.${display.why}`)} ${t('display.nextStart')}`}
      more={t('display.more')}
    >
      <span className="voice__row" role="radiogroup" aria-label={t('display.title')}>
        {CHOICES.map((one) => (
          <button key={one.own ?? 'auto'} className="btn" data-on={display.chosen === one.value ? 'true' : undefined} role="radio" aria-checked={display.chosen === one.value} onClick={() => pick(one.value)}>
            {one.own ?? t('display.automatic')}
          </button>
        ))}
      </span>
    </PrefRow>
  )
}

/* A language's name is said in itself whichever language the screen is in;
   `System` is not a language, so only it translates. */
const LANGS: { value: Language; own: string | null }[] = [
  { value: 'system', own: null },
  { value: 'en', own: 'English' },
  { value: 'pt_br', own: 'Português (Brasil)' },
]

export function LanguageSettings(): React.JSX.Element {
  const [stored, setStored] = useState<Stored | null>(null)
  useLocale()

  useEffect(() => {
    const read = (): void => void ask(() => commands.settingsRead()).then((answer) => setStored(answer.data))
    read()
    /* Another computer's choice, synced here, is read like this one's. */
    return onEvent('sync:written', read)
  }, [])

  const pick = (value: Language): void =>
    void inOrder('settings', () =>
      ask(() => commands.settingsWrite(null, value, null, null, null, null, null, null)),
    ).then((answer) => {
      if (!answer.data) return
      setStored(answer.data)
      /* The answer carries the locale the core resolved; the words follow
         it now, not at the next start. */
      setLocale(answer.data.locale)
    })

  return (
    <PrefRow title={t('language.title')} said={t('language.said')}>
      <span className="voice__row" role="radiogroup" aria-label={t('language.title')}>
        {LANGS.map((one) => (
          <button key={one.value} className="btn" data-on={stored?.language === one.value ? 'true' : undefined} role="radio" aria-checked={stored?.language === one.value} onClick={() => pick(one.value)}>
            {one.own ?? t('language.system')}
          </button>
        ))}
      </span>
    </PrefRow>
  )
}
