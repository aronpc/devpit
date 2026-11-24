import { useCallback, useEffect, useState } from 'react'

import { ask, commands } from './live'
import { inOrder } from './inOrder'
import { onEvent } from './window'
import type { Theme } from './shape'

/*
 * The theme, and the settings read once at start.
 *
 * The choice is written where the next launch will find it; the window paints
 * from local state so the click does not wait on disk. `confirmStop` rides on
 * the same read, and is handed to whoever keeps it.
 */
export function useTheme(setConfirmStop: (asked: boolean | null) => void): {
  theme: Theme
  setTheme: (next: Theme) => void
} {
  const [theme, setThemeState] = useState<Theme>('system')

  const setTheme = useCallback((next: Theme) => {
    setThemeState(next)
    document.documentElement.dataset.theme = next
    void inOrder('settings', () => ask(() => commands.settingsWrite(next, null, null, null, null, null, null, null)))
  }, [])

  useEffect(() => {
    document.documentElement.dataset.theme = 'system'
    const read = (): void =>
      void ask(() => commands.settingsRead()).then((asked) => {
        if (!asked.data) return
        setThemeState(asked.data.theme)
        /* Null is "never asked", and never-asked asks. */
        setConfirmStop(asked.data.confirmStop)
      })
    read()
    /* Another computer's choice, synced here, is read like this one's. */
    return onEvent('sync:written', read)
  }, [setConfirmStop])

  return { theme, setTheme }
}
