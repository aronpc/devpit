import { useEffect, useState } from 'react'

import type { Tray } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { PrefRow } from './PrefRow'
import { PrefSwitch } from './PrefSwitch'
import { abandoned } from './typing'

/*
 * devpit as a desktop app: opening when you log in, staying in the tray when
 * the window closes, and the keys that bring it forward from anywhere.
 */

/** The keys of a key press, as the shortcut plugin spells them, or null while
 *  only modifiers are down. */
export function keysOf(event: Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>): string | null {
  if (['Control', 'Meta', 'Alt', 'Shift', 'OS'].includes(event.key)) return null
  const parts: string[] = []
  if (event.ctrlKey || event.metaKey) parts.push('CommandOrControl')
  if (event.altKey) parts.push('Alt')
  if (event.shiftKey) parts.push('Shift')
  /* A shortcut that is one bare key would take that key from every app. */
  if (parts.length === 0) return null
  const code = event.code
  const key = code.startsWith('Key') ? code.slice(3) : code.startsWith('Digit') ? code.slice(5) : code
  parts.push(key)
  return parts.join('+')
}

export function DeskSettings(): React.JSX.Element {
  const [atLogin, setAtLogin] = useState(false)
  const [tray, setTray] = useState<Tray | null>(null)
  const [shortcut, setShortcut] = useState<string | null>(null)
  const [recording, setRecording] = useState(false)
  const [refused, setRefused] = useState<string | null>(null)

  useEffect(() => {
    void ask(() => commands.atLoginRead()).then((answer) => setAtLogin(answer.data ?? false))
    void ask(() => commands.trayRead()).then((answer) => setTray(answer.data ?? null))
    void ask(() => commands.shortcutRead()).then((answer) => setShortcut(answer.data ?? null))
  }, [])

  useEffect(() => {
    if (!recording) return
    const take = (event: KeyboardEvent): void => {
      event.preventDefault()
      event.stopPropagation()
      if (abandoned(event)) {
        setRecording(false)
        return
      }
      const keys = keysOf(event)
      if (!keys) return
      setRecording(false)
      void ask(() => commands.shortcutSet(keys)).then((answer) => {
        setRefused(answer.error)
        if (!answer.error) setShortcut(answer.data ?? null)
      })
    }
    window.addEventListener('keydown', take, true)
    return () => window.removeEventListener('keydown', take, true)
  }, [recording])

  const clear = (): void => {
    void ask(() => commands.shortcutSet(null)).then((answer) => {
      setRefused(answer.error)
      if (!answer.error) setShortcut(null)
    })
  }

  return (
    <>
      <PrefSwitch
        on={atLogin}
        onFlip={() => void ask(() => commands.atLoginSet(!atLogin)).then((answer) => setAtLogin(answer.data ?? atLogin))}
        title={t('workspace.desk.login')}
        said={t('workspace.desk.loginSaid')}
        more={t('workspace.desk.loginMore')}
      />
      {/* Only where a tray shows the icon: hidden with no way back, the
          window would be a devpit that looks gone and is not. */}
      {tray?.shown && (
        <PrefSwitch
          on={tray.keep}
          onFlip={() => void ask(() => commands.trayKeepSet(!tray.keep)).then((answer) => answer.data && setTray(answer.data))}
          title={t('workspace.desk.tray')}
          said={t('workspace.desk.traySaid')}
          more={t('workspace.desk.trayMore')}
        />
      )}
      {tray && !tray.shown && (
        <PrefRow
          title={t('workspace.desk.tray')}
          said={t('workspace.desk.noTray')}
        />
      )}
      <PrefRow
        title={t('workspace.desk.shortcut')}
        said={
          <>
            {recording ? t('workspace.desk.press') : shortcut ? t('workspace.desk.brings', { keys: shortcut }) : t('workspace.desk.none')}
            {refused && ` ${refused}`}
          </>
        }
      >
        <button className="btn" onClick={() => setRecording((was) => !was)}>
          {recording ? t('workspace.desk.stop') : shortcut ? t('workspace.desk.change') : t('workspace.desk.choose')}
        </button>
        {shortcut && !recording && (
          <button className="btn" onClick={clear}>
            {t('workspace.desk.clear')}
          </button>
        )}
      </PrefRow>
    </>
  )
}
