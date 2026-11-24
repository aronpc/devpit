import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { KEYS_SHEET } from './keysSheet'
import { useRebound } from './rebound'
import { keysLabels, doesOf, SCOPES, scopeTitle, SHORTCUT_LIST } from './shortcuts'
import { useShellPick } from './shellStore'
import { abandoned } from './typing'
import { useShortcut } from './useShortcut'

/*
 * Every shortcut at a glance, drawn from the registry as it is bound now, so
 * a key recorded in Settings is the key the sheet prints.
 */

/** A shortcut's keys, each in its own cap. */
export function Keys({ labels }: { labels: readonly string[] }): React.JSX.Element {
  return (
    <span className="kmap__keys">
      {labels.map((label) => (
        <kbd key={label} className="kmap__key">
          {label}
        </kbd>
      ))}
    </span>
  )
}

/** The sheet's body: the registry, grouped by where each key works. */
export function KeysSheet(): React.JSX.Element {
  useRebound()
  return (
    <div className="ksheet__cols">
      {SCOPES.map(({ scope }) => (
        <section key={scope} className="ksheet__g" aria-label={scopeTitle(scope)}>
          <h3 className="ksheet__gt">{scopeTitle(scope)}</h3>
          {SHORTCUT_LIST.filter((one) => one.scope === scope).map((one) => (
            <div key={one.id} className="ksheet__r">
              <span className="ksheet__d">{doesOf(one)}</span>
              <Keys labels={keysLabels(one.id)} />
            </div>
          ))}
        </section>
      ))}
    </div>
  )
}

export function CheatSheet(): React.JSX.Element | null {
  const [open, setOpen] = useState(false)
  const openPrefs = useShellPick((shell) => shell.openPrefs)
  useShortcut('shortcuts', () => setOpen((was) => !was))

  useEffect(() => {
    const show = (): void => setOpen(true)
    window.addEventListener(KEYS_SHEET, show)
    return () => window.removeEventListener(KEYS_SHEET, show)
  }, [])

  /* Escape is the sheet's while it is open: in capture, so neither the
     dialog behind it nor an agent's double-Escape stop hears it too. The
     keyboard goes back to where it was. */
  useEffect(() => {
    if (!open) return
    const back = document.activeElement
    const key = (event: KeyboardEvent): void => {
      if (!abandoned(event)) return
      event.preventDefault()
      event.stopPropagation()
      setOpen(false)
    }
    window.addEventListener('keydown', key, true)
    return () => {
      window.removeEventListener('keydown', key, true)
      if (back instanceof HTMLElement && back.isConnected) back.focus()
    }
  }, [open])

  if (!open) return null
  return (
    <div className="ask" data-open="true" onClick={(event) => event.target === event.currentTarget && setOpen(false)}>
      <div className="ask__box ksheet" role="dialog" aria-modal="true" aria-label={t('workspace.cheat.title')}>
        <h2 className="ask__t">{t('workspace.cheat.title')}</h2>
        <KeysSheet />
        <div className="ask__row">
          <button
            className="btn"
            onClick={() => {
              setOpen(false)
              openPrefs('keyboard')
            }}
          >
            {t('workspace.cheat.change')}
          </button>
          <button className="btn btn--go" autoFocus onClick={() => setOpen(false)}>
            {t('workspace.cheat.close')}
          </button>
        </div>
      </div>
    </div>
  )
}
