import { useEffect, useState } from 'react'

import type { AppInfo } from '../gen/bindings'
import { t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
import { ask, commands } from './live'
import { went } from './problems'

/* Settings → About: which devpit this is, and where its privacy policy and
   licences are read. */

const LINKS: readonly (readonly [string, string, string])[] = [
  ['about.privacy', 'about.privacySaid', 'https://devpit.app/privacy'],
  ['about.licence', 'about.licenceSaid', 'https://github.com/jholhewres/devpit/blob/main/LICENSE'],
  ['about.notices', 'about.noticesSaid', 'https://github.com/jholhewres/devpit/blob/main/NOTICE'],
]

export function AboutSettings(): React.JSX.Element {
  const [info, setInfo] = useState<AppInfo | null>(null)
  /* Hears the locale, so the rows re-say themselves when it changes. */
  useLocale()

  useEffect(() => {
    void ask(() => commands.appInfo()).then((answer) => setInfo(answer.data))
  }, [])
  return (
    <>
      <div className="pref">
        <span className="pref__body">
          <span className="pref__t">{t('about.version')}</span>
          <span className="pref__d">{info ? `devpit ${info.version}${info.dev ? ' (dev)' : ''} · ${info.platform}` : t('about.reading')}</span>
        </span>
      </div>
      {LINKS.map(([title, said, url]) => (
        <div className="pref" key={title}>
          <span className="pref__body">
            <span className="pref__t">{t(title)}</span>
            <span className="pref__d">{t(said)}</span>
          </span>
          <span className="prefs__acts">
            <button className="btn" onClick={() => void ask(() => commands.urlOpen(url)).then(went)}>
              {t('about.open')}
            </button>
          </span>
        </div>
      ))}
    </>
  )
}
