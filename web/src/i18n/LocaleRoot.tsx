import { createContext, useContext, useEffect, useSyncExternalStore, type ReactNode } from 'react'

import { ask, commands } from '../shell/live'
import { onEvent } from '../shell/window'
import { currentLocale, setLocale, subscribe } from './index'

/*
 * The language, resolved once in the core (T3) and put in front of the web
 * here: the document says it, and whoever renders hears it change. The web
 * never reads `navigator.language` — the choice is the person's, made in
 * Settings, and this only carries what the core resolved.
 */
const LocaleContext = createContext<string>('en')

export function LocaleRoot({ children }: { children: ReactNode }): React.JSX.Element {
  const locale = useSyncExternalStore(subscribe, currentLocale)

  useEffect(() => {
    const read = (): void =>
      void ask(() => commands.settingsRead()).then((asked) => {
        if (asked.data) setLocale(asked.data.locale)
      })
    read()
    /* Another computer's choice, synced here, is read like this one's. */
    return onEvent('sync:written', read)
  }, [])

  useEffect(() => {
    document.documentElement.lang = locale
  }, [locale])

  return <LocaleContext.Provider value={locale}>{children}</LocaleContext.Provider>
}

/** The locale the app is speaking, for a part that renders words in it. */
export function useLocale(): string {
  return useContext(LocaleContext)
}
