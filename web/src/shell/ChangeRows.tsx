import { useMemo, useState } from 'react'

import type { Change } from '../gen/bindings'
import { t } from '../i18n'
import { ChangeMenu } from './ChangeMenu'
import { menuPoint } from './menuRules'
import { bare, foldered, folders, paths, type Node } from './changeTree'
import { FileGlyph } from './FileGlyph'
import { Minus, Plus, Trash, Undo } from './GitIcons'
import { mark } from './tree'
import { Icon } from './Icon'

/*
 * The changed files, drawn as the folders they are in.
 *
 * A flat list is fine for three files and unreadable for forty: `mod.rs` four
 * times over tells you nothing, and the whole path on every row is a column of
 * repeated prefixes in a panel narrow enough to be a sidebar.
 *
 * Open by default, and that is the point of the folding in `changeTree`:
 * somebody opening this panel wants to see what changed, not to go looking for
 * it. Closing is for putting aside a folder you have already read.
 *
 * Or as a list, as Orca offers: each file once, its folder dimmed beside its
 * name. Better when the changes are few and far apart.
 */

export type View = 'tree' | 'list'

export function ChangeRows({
  changes,
  view = 'tree',
  staged,
  busy,
  onOpen,
  onScreen,
  onStage,
  onDiscard,
}: {
  changes: readonly Change[]
  view?: View
  /** Whether these are already in the index, which decides what the button on
   *  a row does. */
  staged: boolean
  busy: boolean
  onOpen: (path: string) => void
  /** The file whose diff is on screen, or null when none is. */
  onScreen?: string | null
  onStage: (paths: string[]) => void
  onDiscard: (change: Change) => void
}): React.JSX.Element {
  const tree = useMemo(
    () => (view === 'tree' ? foldered(changes) : changes.map(asFile)),
    [changes, view],
  )
  const [shut, setShut] = useState<ReadonlySet<string>>(() => new Set())
  const [menu, setMenu] = useState<{ change: Change; x: number; y: number } | null>(null)

  const rows = (nodes: readonly Node[], depth: number): React.JSX.Element[] =>
    nodes.flatMap((node) => {
      if (node.kind === 'file') {
        const { change } = node
        const pad = { paddingLeft: `${20 + depth * 12}px` }
        const folder = view === 'list' ? bare(change.path).split('/').slice(0, -1).join('/') : ''
        return [
          <div
            className="gitrow gitrow--file"
            key={change.path}
            aria-current={change.path === onScreen ? 'true' : undefined}
            onContextMenu={(event) => {
              event.preventDefault()
              setMenu({ change, ...menuPoint(event) })
            }}
          >
            <button className="gitrow__open" style={pad} onClick={() => onOpen(change.path)}>
              {/* Coloured by the git status, so the shape says what kind of
                  file and the colour says what happened to it. */}
              <span className={`row__g--${change.status}`}>
                <FileGlyph path={change.path} />
              </span>
              <span className="gitrow__n">{node.name}</span>
              {folder && <span className="gitrow__dir">{folder}</span>}
            </button>
            <span className="gitrow__end">
              {change.added > 0 && <span className="add">+{change.added}</span>}
              {change.removed > 0 && <span className="del">&minus;{change.removed}</span>}
              <span className={`row__g row__g--${change.status}`}>{mark(change.status)}</span>
            </span>
            {/* Over the row and only while it is under the pointer. Two
                buttons on every one of forty rows is eighty things competing
                with the names, which are what the panel is for. */}
            <span className="gitrow__does">
              <button
                className="gitrow__act"
                disabled={busy}
                onClick={() => onDiscard(change)}
                title={change.status === 'untracked' ? t('workspace.changes.deleteOne') : t('workspace.changes.discardOne')}
                aria-label={t('workspace.changes.discardFileAria', { path: change.path })}
              >
                {change.status === 'untracked' ? <Trash /> : <Undo />}
              </button>
              <button
                className="gitrow__act"
                disabled={busy}
                onClick={() => onStage([change.path])}
                title={staged ? t('workspace.changes.takeOut') : t('workspace.changes.putIn')}
                aria-label={t(staged ? 'workspace.changes.unstageFileAria' : 'workspace.changes.stageFileAria', { path: change.path })}
              >
                {staged ? <Minus /> : <Plus />}
              </button>
            </span>
          </div>,
        ]
      }

      const open = !shut.has(node.path)
      const pad = { paddingLeft: `${8 + depth * 12}px` }
      return [
        <div className="gitrow gitrow--dir" key={node.path}>
          <button
            className="gitrow__open"
            style={pad}
            aria-expanded={open}
            onClick={() =>
              setShut((was) => {
                const next = new Set(was)
                if (open) next.add(node.path)
                else next.delete(node.path)
                return next
              })
            }
          >
            <span className="gitrow__chev" data-open={open}>
              <Icon size="s">
                <path d="m9 18 6-6-6-6" />
              </Icon>
            </span>
            <span className="gitrow__n">{node.name}</span>
          </button>
          <span className="gitrow__end">
            <span className="gitrow__c">{node.files}</span>
          </span>
          {/* A folder acts on everything under it, which is the click this
              panel was missing: staging nine files one at a time is nine
              clicks for one decision. */}
          <span className="gitrow__does">
            <button
              className="gitrow__act"
              disabled={busy}
              onClick={() => onStage(paths([node]))}
              title={staged ? t('workspace.changes.takeAllOut') : t('workspace.changes.putAllIn')}
              aria-label={t(staged ? 'workspace.changes.unstageEverythingIn' : 'workspace.changes.stageEverythingIn', { path: node.path })}
            >
              {staged ? <Minus /> : <Plus />}
            </button>
          </span>
        </div>,
        ...(open ? rows(node.children, depth + 1) : []),
      ]
    })

  return (
    <>
      {/* Nothing to collapse when the tree is one row deep. */}
      {folders(tree).length > 1 && (
        <button
          className="gitrow__all"
          onClick={() =>
            setShut((was) => (was.size > 0 ? new Set() : new Set(folders(tree))))
          }
        >
          {shut.size > 0 ? t('workspace.changes.expandAll') : t('workspace.changes.collapseAll')}
        </button>
      )}
      {rows(tree, 0)}
      {menu && (
        <ChangeMenu
          change={menu.change}
          at={menu}
          staged={staged}
          busy={busy}
          onOpen={onOpen}
          onStage={onStage}
          onDiscard={onDiscard}
          onClose={() => setMenu(null)}
        />
      )}
    </>
  )
}

const asFile = (change: Change): Node => ({
  kind: 'file',
  name: bare(change.path).split('/').pop() ?? change.path,
  change,
})
