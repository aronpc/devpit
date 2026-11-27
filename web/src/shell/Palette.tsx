import { useEffect, useMemo, useRef, useState } from 'react'

import { t } from '../i18n'
import { useGroups } from './paletteGroups'
import { TALK } from './paletteIcons'
import { openHit, useHits } from './SessionSearch'
import { pieces } from './snippet'
import { PaletteKeys, PaletteList } from './PaletteList'
import { useReachable } from './paletteReach'
import { useShell } from './useShell'
import { committed } from './typing'
import { Icon } from './Icon'

/*
 * One field over the window, for reaching what the window has.
 *
 * Panes, cards, files and the sessions that are open. The file list is
 * fetched once when the field opens and filtered in memory: a search that
 * walks the disk on every keystroke is a search nobody leaves open.
 */

export function Palette({ onClose }: { onClose: () => void }): React.JSX.Element {
  const { show, openChat, projects } = useShell()
  const { panes, agents, sessions, cards, files, indexing, partial } = useReachable()
  const [query, setQuery] = useState('')
  const [at, setAt] = useState(0)
  const field = useRef<HTMLInputElement>(null)

  useEffect(() => field.current?.focus(), [])

  /* What was said in any project's conversations, opened in its own project. */
  const said = useHits(query, null, query.trim().length > 1).hits
  const conversations = useMemo(
    () =>
      said.map((hit, index) => ({
        key: `said:${hit.projectId}:${hit.sessionId}:${index}`,
        name: pieces(hit.snippet).map((piece) => piece.text).join(''),
        meta: projects.find((one) => one.id === hit.projectId)?.name ?? '',
        icon: TALK,
        go: () => void openHit(hit, openChat),
      })),
    [said, projects, openChat],
  )

  const groups = useGroups({ panes, agents, sessions, conversations, cards, files, query, show })

  const flat = groups.flatMap(([, rows]) => rows)

  function go(index: number): void {
    const row = flat[index]
    if (!row) return
    onClose()
    row.go()
  }

  function onKey(event: React.KeyboardEvent): void {
    if (!flat.length) return
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      setAt((was) => (was + 1) % flat.length)
    } else if (event.key === 'ArrowUp') {
      event.preventDefault()
      setAt((was) => (was - 1 + flat.length) % flat.length)
    } else if (committed(event)) {
      event.preventDefault()
      go(at)
    }
  }

  return (
    <div className="cmd" data-open="true" onClick={(event) => event.target === event.currentTarget && onClose()}>
      <div className="cmd__box" role="dialog" aria-label={t('workspace.palette.search')}>
        <div className="cmd__q">
          <Icon size="m">
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.3-4.3" />
          </Icon>
          <input
            ref={field}
            className="cmd__in"
            type="text"
            autoComplete="off"
            spellCheck={false}
            placeholder={t('workspace.palette.placeholder')}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value)
              setAt(0)
            }}
            onKeyDown={onKey}
          />
        </div>

        <PaletteList
          groups={groups}
          at={at}
          indexing={indexing}
          partial={partial}
          onHover={setAt}
          onPick={go}
        />

        <PaletteKeys />
      </div>
    </div>
  )
}
