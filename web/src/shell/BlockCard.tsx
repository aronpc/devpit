import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { memo, useEffect, useState } from 'react'

import type { CommandBlock } from '../gen/bindings'
import { plain, rendered, type Line } from './blockRender'
import { filtered, linked, outcome, shortPath, took } from './blockText'
import { Bookmark, Copy, Pencil, Search, Undo } from './GitIcons'
import { ask, commands } from './live'
import { RailMenu, type RailItem } from './RailMenu'
import { darkNow, palette } from './terminal'
import { menuPoint } from './menuRules'
import { t } from '../i18n'

/*
 * One command, as a block: what ran, where, how long, how it ended — and
 * what it printed, drawn back from its own bytes.
 *
 * The header sticks to the top of the list while its output scrolls under
 * it, so a long build still says which command it is. Its actions are the
 * ones a person reaches for after a command: copy it, copy what it said, run
 * it again, change it and run that, look for a line in it.
 */

/* Drawn once per block and width, and kept: a list scrolled back through
   should not run a terminal per block every time it re-renders. Bounded, the
   oldest going first: a day of commands at a few widths would otherwise all
   stay in memory. */
const drawn = new Map<string, readonly Line[]>()
const MOST_DRAWN = 300
function recall(key: string): readonly Line[] | null {
  const lines = drawn.get(key)
  if (!lines) return null
  keep(key, lines)
  return lines
}
function keep(key: string, lines: readonly Line[]): void {
  drawn.delete(key)
  drawn.set(key, lines)
  while (drawn.size > MOST_DRAWN) drawn.delete(drawn.keys().next().value as string)
}

/* Past this many lines the block shows its end and offers the rest. */
const FIRST_SHOWN = 400

export const BlockCard = memo(function BlockCard({
  paneId,
  block,
  cols,
  home,
  jumped,
  onRerun,
  onEdit,
  onBookmark,
}: {
  paneId: string
  block: CommandBlock
  cols: number
  home: string | null
  /** The block the last Alt+↑/↓ landed on. */
  jumped: boolean
  onRerun: (line: string) => void
  onEdit: (line: string) => void
  /** Stable across renders, so `memo` holds: the block is named here. */
  onBookmark: (id: number | null, on: boolean) => void
}): React.JSX.Element {
  const key = `${paneId}:${block.id}:${cols}`
  /* Drawn for one width: a new width reads the cache or draws again, and the
     lines of the old one stay on screen until the new ones are ready. */
  const [drawnFor, setDrawnFor] = useState<{ key: string; lines: readonly Line[] } | null>(() => {
    const cached = recall(key)
    return cached ? { key, lines: cached } : null
  })
  const current = drawnFor?.key === key ? drawnFor.lines : recall(key)
  const lines = current ?? drawnFor?.lines ?? null
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)
  const [query, setQuery] = useState<string | null>(null)
  const [all, setAll] = useState(false)
  const [open, setOpen] = useState(true)
  const state = outcome(block)

  useEffect(() => {
    if (current || block.interactive) return
    let live = true
    void ask(() => commands.blockOutput(paneId, block.id)).then(async (answer) => {
      if (!live || answer.data === null) return
      const next = await rendered(answer.data, cols, palette(darkNow()))
      keep(key, next)
      if (live) setDrawnFor({ key, lines: next })
    })
    return () => {
      live = false
    }
  }, [current, block.interactive, block.id, paneId, cols, key])

  const text = lines ? plain(lines) : ''
  const kept = lines && query !== null ? filtered(text.split('\n'), query) : null
  const visible = lines ? (kept ? kept.map((at) => lines[at]!) : lines) : []
  const clipped = !all && !kept && visible.length > FIRST_SHOWN
  const shown = clipped ? visible.slice(-FIRST_SHOWN) : visible
  const line = block.command ?? ''

  const items: RailItem[] = [
    { label: t('workspace.blockCard.copyCommand'), glyph: <Copy />, act: () => void writeText(line) },
    { label: t('workspace.blockCard.copyOutput'), glyph: <Copy />, act: () => void writeText(text) },
    { label: t('workspace.blockCard.copyBoth'), glyph: <Copy />, act: () => void writeText(`${line}\n${text}`) },
    'rule',
    { label: t('workspace.blockCard.runAgain'), glyph: <Undo />, act: () => onRerun(line) },
    { label: t('workspace.blockCard.editRun'), glyph: <Pencil />, act: () => onEdit(line) },
    { label: query === null ? t('workspace.blockCard.filter') : t('workspace.blockCard.stopFilter'), glyph: <Search />, act: () => setQuery(query === null ? '' : null) },
    'rule',
    { label: block.bookmarked ? t('workspace.blockCard.unbookmark') : t('workspace.blockCard.bookmark'), glyph: <Bookmark />, act: () => onBookmark(block.id, !block.bookmarked) },
  ]

  return (
    <article className="blk" data-block-id={block.id} data-outcome={state} data-open={open ? 'true' : 'false'} data-jumped={jumped ? 'true' : undefined}>
      <header
        className="blk__head"
        onClick={() => setOpen(!open)}
        onContextMenu={(event) => {
          event.preventDefault()
          setMenu(menuPoint(event))
        }}
      >
        <span className="blk__dot" aria-label={state} />
        {block.bookmarked && (
          <span className="blk__mark" title={t('workspace.blockCard.bookmarked')}>
            <Bookmark />
          </span>
        )}
        <code className="blk__cmd" title={line}>
          {line || <span className="blk__nocmd">{t('workspace.blockCard.noCommand')}</span>}
        </code>
        <span className="blk__meta">
          {block.cwd && <span className="blk__cwd" title={block.cwd}>{shortPath(block.cwd, home)}</span>}
          {state === 'failed' && <span className="blk__code">{t('workspace.blockCard.exit', { code: block.code ?? '' })}</span>}
          <span>{took(block, Date.now())}</span>
        </span>
        <button
          className="blk__more"
          aria-label={t('workspace.blockCard.actions')}
          title={t('workspace.blockCard.actions')}
          onClick={(event) => {
            event.stopPropagation()
            setMenu(menuPoint(event))
          }}
        >
          ⋯
        </button>
      </header>

      {open && query !== null && (
        <div className="blk__filter">
          <Search />
          <input autoFocus value={query} placeholder={t('workspace.blockCard.filterPlaceholder')} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => event.key === 'Escape' && setQuery(null)} />
          <span>{t('workspace.blockCard.lines', { count: kept?.length ?? 0 })}</span>
        </div>
      )}

      {open && (
        <div className="blk__out">
          {block.interactive ? (
            <div className="blk__note">{t('workspace.blockCard.interactive')}</div>
          ) : lines === null ? (
            <div className="blk__note">…</div>
          ) : (
            <>
              {(clipped || block.truncated) && (
                <button className="blk__all" onClick={() => setAll(true)} disabled={!clipped}>
                  {block.truncated ? t('workspace.blockCard.truncated') : ''}
                  {clipped ? ` ${t('workspace.blockCard.showAll', { count: visible.length })}` : ''}
                </button>
              )}
              <pre className="blk__pre">
                {shown.map((runs, at) => (
                  <div key={at} className="blk__line">
                    {runs.map((run, i) => (
                      <span
                        key={i}
                        style={{
                          color: run.fg,
                          background: run.bg,
                          fontWeight: run.bold ? 600 : undefined,
                          fontStyle: run.italic ? 'italic' : undefined,
                          textDecoration: run.underline ? 'underline' : undefined,
                          opacity: run.dim ? 0.6 : undefined,
                        }}
                      >
                        {linked(run.text).map((piece, p) =>
                          piece.url ? (
                            <a key={p} className="blk__link" href={piece.url} title={t('workspace.blockCard.open', { url: piece.url })} onClick={(event) => {
                              event.preventDefault()
                              void ask(() => commands.urlOpen(piece.url!))
                            }}>
                              {piece.text}
                            </a>
                          ) : (
                            piece.text
                          ),
                        )}
                      </span>
                    ))}
                  </div>
                ))}
              </pre>
            </>
          )}
        </div>
      )}

      {menu && <RailMenu at={menu} items={items} label={t('workspace.blockCard.actions')} onClose={() => setMenu(null)} />}
    </article>
  )
})
