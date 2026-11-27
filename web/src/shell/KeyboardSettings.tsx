import { Fragment, useEffect, useState } from 'react'

import { t } from '../i18n'
import { Keys } from './CheatSheet'
import { useRebound } from './rebound'
import {
  doesOf,
  firesInTerminal,
  isRebound,
  keysLabels,
  recordKeys,
  resetAllKeys,
  resetKeys,
  SCOPES,
  scopeTitle,
  SHORTCUT_LIST,
  whenOf,
  type Shortcut,
  type ShortcutId,
} from './shortcuts'
import { abandoned, composing } from './typing'
import { Icon } from './Icon'

/*
 * Settings → Keyboard: every shortcut, the keys it answers to now, and a way
 * to record others. A recorded key is kept at once and works at once; one
 * that another shortcut already answers to where this one would is refused,
 * and the refusal names that shortcut.
 */

const titleOf = (one: Shortcut): string => scopeTitle(one.scope)

/** The rows a search keeps: by what they do, where they work, or their keys. */
function shortcutsMatching(query: string): readonly Shortcut[] {
  const wanted = query.trim().toLowerCase()
  if (!wanted) return SHORTCUT_LIST
  return SHORTCUT_LIST.filter((one) => [doesOf(one), titleOf(one), whenOf(one) ?? '', ...keysLabels(one.id)].join(' ').toLowerCase().includes(wanted))
}

/* What a row says under its name: where it does not fire, which is the
   surprise worth a line, and when it acts. */
function noteOf(one: Shortcut): string | null {
  const when = whenOf(one)
  if (one.fixed) return [t('workspace.keyboard.builtIn'), when].filter(Boolean).join(' ')
  if (one.scope !== 'global') return when
  return firesInTerminal(one) ? t('workspace.keyboard.inTerminal') : t('workspace.keyboard.toProgram')
}

export function KeyboardSettings({ onGeneral }: { onGeneral: () => void }): React.JSX.Element {
  useRebound()
  const [query, setQuery] = useState('')
  const [recording, setRecording] = useState<ShortcutId | null>(null)
  const [refused, setRefused] = useState<{ id: ShortcutId; why: string } | null>(null)

  /* In capture and stopped there: the keys being recorded must not also do
     what they do now, and Escape stops the recording rather than Settings. */
  useEffect(() => {
    if (!recording) return
    const take = (event: KeyboardEvent): void => {
      if (composing(event)) return
      event.preventDefault()
      event.stopPropagation()
      if (abandoned(event)) return setRecording(null)
      const recorded = recordKeys(recording, event)
      if (!recorded) return
      if ('refused' in recorded) return setRefused({ id: recording, why: recorded.refused })
      setRefused(null)
      setRecording(null)
    }
    window.addEventListener('keydown', take, true)
    return () => window.removeEventListener('keydown', take, true)
  }, [recording])

  const reset = (id: ShortcutId): void => {
    const why = resetKeys(id)
    setRefused(why ? { id, why } : null)
  }

  const shown = shortcutsMatching(query)
  const sheet = keysLabels('shortcuts')

  return (
    <>
      <div className="prefs__hrow">
        <h1 className="prefs__h">{t('prefs.nav.keyboard')}</h1>
        {SHORTCUT_LIST.some((one) => isRebound(one.id)) && (
          <button
            className="btn"
            onClick={() => {
              resetAllKeys()
              setRefused(null)
              setRecording(null)
            }}
          >
            {t('workspace.keyboard.resetAll')}
          </button>
        )}
      </div>
      <p className="acc__note">
        {t('workspace.keyboard.note', { keys: sheet.join(t('workspace.keyboard.or')) })}
      </p>
      <label className="kmap__find">
        <Icon size="s"><circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" /></Icon>
        <input className="prefs__q" value={query} placeholder={t('workspace.keyboard.searchPlaceholder')} spellCheck={false} aria-label={t('workspace.keyboard.searchLabel')} onChange={(event) => setQuery(event.target.value)} />
      </label>

      {SCOPES.map(({ scope }) => {
        const rows = shown.filter((one) => one.scope === scope)
        if (rows.length === 0) return null
        return (
          <Fragment key={scope}>
            <div className="acc__sub">{scopeTitle(scope)}</div>
            <div className="kmap" role="list" aria-label={scopeTitle(scope)}>
              {rows.map((one) => {
                const live = recording === one.id
                const why = refused?.id === one.id ? refused.why : null
                const note = why ?? noteOf(one)
                return (
                  <div key={one.id} className="kmap__row" role="listitem" data-recording={live ? 'true' : undefined}>
                    <span className="kmap__body">
                      <span className="kmap__t">{doesOf(one)}</span>
                      {note && <span className="kmap__d" data-refused={why ? 'true' : undefined}>{note}</span>}
                    </span>
                    {live ? <span className="kmap__wait">{t('workspace.keyboard.press')}</span> : <Keys labels={keysLabels(one.id)} />}
                    {!one.fixed && (
                      <span className="prefs__acts">
                        <button
                          className="btn"
                          onClick={() => {
                            setRefused(null)
                            setRecording(live ? null : one.id)
                          }}
                        >
                          {live ? t('workspace.keyboard.stop') : t('workspace.keyboard.record')}
                        </button>
                        {isRebound(one.id) && !live && (
                          <button className="btn" onClick={() => reset(one.id)}>
                            {t('workspace.keyboard.reset')}
                          </button>
                        )}
                      </span>
                    )}
                  </div>
                )
              })}
            </div>
          </Fragment>
        )
      })}
      {shown.length === 0 && <p className="prefs__none">{t('workspace.keyboard.none', { query: query.trim() })}</p>}

      <div className="acc__sub">{t('workspace.keyboard.anyApp')}</div>
      <div className="pref">
        <span className="pref__body">
          <span className="pref__t">{t('workspace.keyboard.bring')}</span>
          <span className="pref__d">{t('workspace.keyboard.bringSaid')}</span>
        </span>
        <span className="prefs__acts">
          <button className="btn" onClick={onGeneral}>
            {t('workspace.keyboard.openGeneral')}
          </button>
        </span>
      </div>
    </>
  )
}
