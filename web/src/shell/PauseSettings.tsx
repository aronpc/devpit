import { t } from '../i18n'
import { formatDate } from '../i18n/format'
import { useLocale } from '../i18n/LocaleRoot'
import { PrefMore } from './PrefRow'
import { pauseOptions, usePause } from './usePause'

/*
 * The pause, in Settings as well as the status strip and the tray: a thing
 * reachable only from a moon in the corner was a thing people never found,
 * and never learned what it held back.
 */
export function PauseSettings(): React.JSX.Element {
  const { paused, pauseFor, resume } = usePause()
  /* Hears the locale, so the row re-says itself when it changes. */
  useLocale()
  const until = paused.until === null
    ? t('pause.untilResumed')
    : t('pause.untilTime', { time: formatDate(paused.until * 1000, { hour: '2-digit', minute: '2-digit' }) })
  return (
    <div className="pref pref--stack">
      <div className="pref__body">
        <span className="pref__t">{paused.on ? t('pause.pausedTitle', { until }) : t('pause.title')}</span>
        <span className="pref__d">{t('pause.said')}</span>
        <PrefMore>{t('pause.more')}</PrefMore>
        <span className="voice__row">
          {paused.on ? (
            <button className="btn" onClick={resume}>
              {t('pause.resume')}
            </button>
          ) : (
            pauseOptions(Date.now() / 1000).map((option) => (
              <button key={option.id} className="btn" onClick={() => pauseFor(option.until)}>
                {t(`pause.option.${option.id}`)}
              </button>
            ))
          )}
        </span>
      </div>
    </div>
  )
}
