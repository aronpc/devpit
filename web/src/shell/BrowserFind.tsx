import { useEffect, useRef, useState } from 'react'

import { ask, commands } from './live'
import { abandoned, committed } from './typing'
import { t } from '../i18n'

/*
 * Finding text in the page, from a bar that belongs to devpit.
 *
 * The page is a webview of its own and its own Ctrl-F is the engine's — which
 * on WebKitGTK is nothing at all, because a webview is not a browser window.
 * So the shortcut has to be caught here, in the window that draws the pane,
 * and the search run inside the page through `window.find`.
 *
 * **It cannot say `3 of 17`.** `window.find` answers whether it moved, not how
 * many matches exist, and no other way to ask exists from inside the page. So
 * this bar shows the box, Enter and Shift-Enter, and stays quiet about a count
 * it would have to invent.
 */

export function BrowserFind({
  pane,
  onDone,
}: {
  pane: string
  onDone: () => void
}): React.JSX.Element {
  const [what, setWhat] = useState('')
  const field = useRef<HTMLInputElement>(null)

  useEffect(() => field.current?.focus(), [])

  const seek = (backwards: boolean): void => {
    if (!what.trim()) return
    void ask(() => commands.browserFind(pane, what, backwards))
  }

  return (
    <form
      className="bfind"
      role="search"
      onSubmit={(event) => {
        event.preventDefault()
        seek(false)
      }}
    >
      <input
        className="bfind__in"
        ref={field}
        aria-label={t('workspace.browserFind.find')}
        placeholder={t('workspace.browserFind.find')}
        spellCheck={false}
        value={what}
        onChange={(event) => setWhat(event.target.value)}
        onKeyDown={(event) => {
          /* Through the composition guard, not on the raw key: this is a text
             field, and an IME uses Escape to abandon a half-typed word and
             Enter to commit one. Closing the bar or jumping the page on
             either would take the word with it. */
          if (abandoned(event)) onDone()
          /* Shift-Enter walks back, which is what a find bar has always done
             and the only reason `backwards` reaches the command at all. */
          if (committed(event) && event.shiftKey) {
            event.preventDefault()
            seek(true)
          }
        }}
      />
      <button className="browser__act" type="button" aria-label={t('workspace.browserFind.previous')} title={t('workspace.browserFind.previous')} onClick={() => seek(true)}>
        ‹
      </button>
      <button className="browser__act" type="submit" aria-label={t('workspace.browserFind.next')} title={t('workspace.browserFind.next')}>
        ›
      </button>
      <button className="browser__act" type="button" aria-label={t('workspace.browserFind.close')} title={t('workspace.browserFind.close')} onClick={onDone}>
        ✕
      </button>
    </form>
  )
}
