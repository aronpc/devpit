import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
import { CommunityFoot } from './Community'
import { matching } from './prefsNav'
import type { PrefsPane } from './shape'
import { Icon } from './Icon'

/*
 * The settings column: back, a search field, and the panes under headings.
 *
 * The field used to be a `div` that said "Search settings" and did nothing —
 * which is worse than no field, because it is the first thing a person reaches
 * for when they cannot find a setting, and it taught them the app has no
 * search. It filters the list now, on the pane's name and on the words someone
 * would look for instead of that name.
 */

/** The row a search was after, in the pane on screen: one titled with the
 *  words first, then one that says them anywhere, folded help included. */
export function rowFor(query: string, root: ParentNode = document): HTMLElement | null {
  const wanted = query.trim().toLowerCase()
  if (!wanted) return null
  const rows = Array.from(root.querySelectorAll<HTMLElement>('.prefs__in:not([hidden]) .pref'))
  const said = (row: HTMLElement, part: string): boolean => (row.querySelector(part)?.textContent ?? '').toLowerCase().includes(wanted)
  return rows.find((row) => said(row, '.pref__t')) ?? rows.find((row) => (row.textContent ?? '').toLowerCase().includes(wanted)) ?? null
}

/* Frames to wait for a row whose pane is still reading what it draws. */
const LANDING_FRAMES = 60

export function PrefsSide({
  pane,
  row = null,
  onBack,
  onSelect,
}: {
  pane: PrefsPane
  /** The row Settings was opened for, scrolled to once it is drawn. */
  row?: string | null
  onBack: () => void
  onSelect: (pane: PrefsPane) => void
}): React.JSX.Element {
  const [query, setQuery] = useState('')
  /* Hears the locale, so the column re-says itself when it changes. */
  useLocale()
  const groups = matching(query)

  useEffect(() => {
    if (!row) return
    let frame = 0
    let left = LANDING_FRAMES
    const land = (): void => {
      const found = rowFor(row)
      if (found) found.scrollIntoView({ block: 'center' })
      else if (left-- > 0) frame = requestAnimationFrame(land)
    }
    land()
    return () => cancelAnimationFrame(frame)
  }, [pane, row])

  return (
    <aside className="prefs__side">
      <button className="prefs__back" onClick={onBack}>
        <Icon size="m"><path d="M19 12H5M12 19l-7-7 7-7" /></Icon>
        {t('prefs.back')}
      </button>

      <label className="prefs__find">
        <Icon size="s"><circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" /></Icon>
        <input
          className="prefs__q"
          value={query}
          placeholder={t('prefs.search')}
          spellCheck={false}
          aria-label={t('prefs.search')}
          onChange={(event) => setQuery(event.target.value)}
        />
      </label>

      <nav className="prefs__nav">
        {groups.map((group) => (
          <div className="prefs__group" key={group.id}>
            <p className="prefs__gt">{group.title}</p>
            {group.items.map((item) => (
              <button
                className="prefs__i"
                key={item.id}
                aria-selected={pane === item.id}
                onClick={() => {
                  onSelect(item.id)
                  /* A long pane; a search for "tailscale" should land on its row, not on the pane's top. */
                  if (query.trim()) requestAnimationFrame(() => rowFor(query)?.scrollIntoView({ block: 'center' }))
                }}
              >
                {item.icon}
                {item.label}
              </button>
            ))}
          </div>
        ))}
        {groups.length === 0 && (
          <p className="prefs__none">{t('prefs.none', { query: query.trim() })}</p>
        )}
      </nav>

      <CommunityFoot />
    </aside>
  )
}
