import { useEffect, useMemo, useRef, useState } from 'react'

import { FileGlyph } from './FileGlyph'
import { ask, commands } from './live'
import { useShell } from './useShell'
import { useWorkspaceFolder } from './useWorkspace'
import { WorkspaceFile } from './WorkspaceFile'
import { committed } from './typing'
import { crumbs, fullPath, matching, meta, moved, parentOf, sorted, type Order } from './wsbrowse'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * The devpit workspace, browsed.
 *
 * This panel used to draw the project's checkout, which is what the Explorer
 * beside it already draws — two readers of one tree, and the wider one no
 * better at it. The workspace is the half nothing showed: every session's
 * transcript, the worktree a card actually runs in, the agents this machine
 * has. All on disk, all named by ULID, and until now all unreachable without
 * a terminal.
 *
 * A navigator rather than a tree, because a tree answers "what is under this"
 * and the question here is "what is in here": the folders are deep, their
 * names are opaque, and the crumbs are what make them readable.
 */

const SORTS: readonly (readonly [Order, string])[] = [
  ['name', 'workspace.files.sortName'],
  ['size', 'workspace.files.sortSize'],
  ['modified', 'workspace.files.sortChanged'],
]

export function FilesPane(): React.JSX.Element {
  const { project, close } = useShell()
  const folder = useWorkspaceFolder(project?.id ?? null)
  const [query, setQuery] = useState('')
  const [order, setOrder] = useState<Order>('name')
  const [picked, setPicked] = useState<string | null>(null)

  const listing = folder.listing
  const here = listing?.path ?? ''
  const root = listing?.root ?? ''
  const up = parentOf(here)

  const rows = useMemo(
    () => sorted(matching(listing?.entries ?? [], query), order),
    [listing, query, order],
  )
  const at = rows.findIndex((row) => row.path === picked)

  /* The row that was clicked to come in here has just unmounted, and focus
     with it — the keyboard would land on the document and the arrows would do
     nothing until something was clicked again. */
  const list = useRef<HTMLDivElement>(null)
  const came = useRef(here)
  useEffect(() => {
    if (came.current === here) return
    came.current = here
    list.current?.focus()
  }, [here])

  const enter = (path: string, isDir: boolean): void => {
    if (isDir) {
      setPicked(null)
      setQuery('')
      folder.go(path)
      return
    }
    setPicked(path)
  }

  const folders = rows.filter((row) => row.isDir).length

  return (
    <>
      <div className="pane__bar">
        <span className="pane__ico">
          <Icon size="s"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /></Icon>
        </span>
        <span className="pane__t">
          <b>{t('workspace.files.title')}</b>
          {project ? ` · ${project.name}` : ''}
        </span>
        <span className="drag"></span>
        <button
          className="sq26"
          onClick={() => void ask(() => commands.pathReveal(fullPath(root, here)))}
          disabled={!root}
          aria-label={t('workspace.files.reveal')}
          title={t('workspace.files.reveal')}
        >
          <Icon size="s"><path d="M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" /></Icon>
        </button>
        <button className="sq26" onClick={folder.reload} aria-label={t('workspace.files.refresh')} title={t('workspace.files.refresh')}>
          <Icon size="s"><path d="M21 12a9 9 0 0 1-15.5 6.2M3 12a9 9 0 0 1 15.5-6.2M3 20v-5h5M21 4v5h-5" /></Icon>
        </button>
        <button className="sq26" onClick={() => close('files')} aria-label={t('workspace.files.close')} title={t('workspace.files.close')}>
          <Icon size="s"><path d="M18 6 6 18M6 6l12 12" /></Icon>
        </button>
      </div>

      <div className="fb" data-preview={String(picked !== null)}>
        <div className="fb__main">
          <div className="fb__head">
            <nav className="fb__crumbs" aria-label={t('workspace.files.folder')}>
              <button
                className="sq26"
                onClick={() => up !== null && folder.go(up)}
                disabled={up === null}
                aria-label={t('workspace.files.up')}
                title={t('workspace.files.up')}
              >
                <Icon size="s"><path d="M12 19V5M5 12l7-7 7 7" /></Icon>
              </button>
              {crumbs(here, project ?? null).map((crumb, index, all) => (
                <span className="fb__crumb" key={crumb.path}>
                  {index > 0 && <span className="fb__sep">/</span>}
                  <button
                    className="fb__up"
                    aria-current={index === all.length - 1}
                    onClick={() => folder.go(crumb.path)}
                  >
                    {crumb.label}
                  </button>
                </span>
              ))}
            </nav>

            {listing && listing.places.length > 0 && (
              <div className="fb__places">
                {listing.places.map((place) => (
                  <button
                    className="fb__place"
                    key={place.path}
                    aria-current={here === place.path}
                    onClick={() => folder.go(place.path)}
                  >
                    {place.label}
                  </button>
                ))}
              </div>
            )}

            <div className="fb__tools">
              <input
                className="fb__find"
                placeholder={t('workspace.files.filterPlaceholder')}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                aria-label={t('workspace.files.filter')}
              />
              <div className="seg seg--tight" role="tablist" aria-label={t('workspace.files.sortBy')}>
                {SORTS.map(([key, label]) => (
                  <button
                    className="segb"
                    key={key}
                    role="tab"
                    aria-selected={order === key}
                    onClick={() => setOrder(key)}
                  >
                    {t(label)}
                  </button>
                ))}
              </div>
            </div>
          </div>

          <div
            className="fb__list"
            ref={list}
            /* One tab stop for the list, the way the tree does it: a folder of
               four hundred rows is four hundred stops otherwise. */
            tabIndex={0}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
                event.preventDefault()
                const next = moved(at, event.key === 'ArrowDown' ? 1 : -1, rows.length)
                setPicked(rows[next]?.path ?? null)
                return
              }
              if (committed(event) && rows[at]) {
                event.preventDefault()
                enter(rows[at].path, rows[at].isDir)
                return
              }
              if (event.key === 'Backspace' && up !== null) {
                event.preventDefault()
                folder.go(up)
              }
            }}
          >
            {folder.error && <div className="exempty__t">{folder.error}</div>}

            {!folder.error && !folder.loading && rows.length === 0 && (
              <div className="exempty">
                <span className="exempty__t">
                  {query ? t('workspace.files.noMatch') : t('workspace.files.empty')}
                </span>
                <span className="exempty__d">
                  {query
                    ? t('workspace.files.clearFilter')
                    : t('workspace.files.writesHere')}
                </span>
              </div>
            )}

            {rows.map((row) => (
              <button
                className="fb__row"
                key={row.path}
                aria-current={row.path === picked}
                onClick={() => enter(row.path, row.isDir)}
                title={fullPath(root, row.path)}
                tabIndex={-1}
              >
                <span className="fb__ico">
                  {row.isDir ? (
                    <Icon size="s"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /></Icon>
                  ) : (
                    <FileGlyph path={row.name} />
                  )}
                </span>
                <span className="fb__n">{row.name}</span>
                <span className="fb__m">{meta(row)}</span>
                {row.isDir && (
                  <span className="fb__go">
                    <Icon size="s"><path d="m9 6 6 6-6 6" /></Icon>
                  </span>
                )}
              </button>
            ))}
          </div>

          <div className="fb__foot">
            <span className="fb__count">
              {rows.length === 0
                ? t('workspace.files.nothing')
                : t('workspace.files.tally', { folders: t('workspace.files.folders', { count: folders }), files: t('workspace.files.files', { count: rows.length - folders }) })}
            </span>
            <span className="fb__where">
              {/* Isolated: the box runs right-to-left so the ellipsis eats the
                  start of the path, and without this the leading `/` is drawn
                  at the end. */}
              <bdi>{fullPath(root, here)}</bdi>
            </span>
          </div>
        </div>

        {/* A folder picked with the arrows is only highlighted; the preview is
            for files, and a folder is entered with Enter. */}
        {picked && !rows.find((row) => row.path === picked)?.isDir && (
          <WorkspaceFile
            path={picked}
            full={fullPath(root, picked)}
            onClose={() => setPicked(null)}
          />
        )}
      </div>
    </>
  )
}
