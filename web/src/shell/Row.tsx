import { useEffect, useRef, useState } from 'react'

import { t } from '../i18n'
import { formatNumber } from '../i18n/format'
import { ask, commands } from './live'

import type { FileNode } from '../gen/bindings'
import { mark, ordered, refreshChildren } from './tree'
import { useRowDrag } from './useRowDrag'
import { Icon } from './Icon'

/* A folder with a few thousand entries would build a few thousand buttons.
   Capped the same way `SearchResults` caps a match list — the true count is
   always shown, just not always rendered — except here "show more" reveals
   the next batch rather than the rest all at once. */
const MOST_SHOWN = 300

const Chev = ({ open }: { open: boolean }): React.JSX.Element => (
  <Icon size="s" style={{ transform: open ? 'rotate(90deg)' : 'none' }}>
    <path d="m9 6 6 6-6 6" />
  </Icon>
)

const Doc = (): React.JSX.Element => (
  <Icon size="s">
    <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
    <path d="M14 2v5h5" />
  </Icon>
)

const Dir = (): React.JSX.Element => (
  <Icon size="s">
    <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />
  </Icon>
)

export function Row({
  projectId,
  node,
  version,
  depth,
  keep,
  filtering,
  current,
  collapsed,
  onOpen,
}: {
  projectId: string
  node: FileNode
  version: number
  depth: number
  keep: Set<string>
  filtering: boolean
  current: string | null
  collapsed: number
  onOpen: (path: string) => void
}): React.JSX.Element | null {
  /* `collapsed` is a counter, not a flag: Collapse all bumps it and every row
     re-reads it, which shuts folders the reader opened by hand. */
  const [open, setOpen] = useState(false)
  const [shutAt, setShutAt] = useState(collapsed)
  /* The tree arrives a level at a time, so a folder fetches its own children
     the first time it is opened. */
  const [children, setChildren] = useState<readonly FileNode[] | null>(
    node.children?.length ? node.children : null,
  )
  const [shown, setShown] = useState(MOST_SHOWN)
  const folded = collapsed !== shutAt ? false : open

  /* A folder's children come from its own fetch; a reload only refreshes the
     root. `refreshChildren` (tree.ts) says what a version bump means here.
     Skipped on mount — the initial `version` is not a reload. */
  const seenVersion = useRef(version)
  useEffect(() => {
    if (version === seenVersion.current) return
    seenVersion.current = version
    const action = refreshChildren(children !== null, folded)
    if (action === 'drop') return setChildren(null)
    if (action === 'now')
      void ask(() => commands.projectTree(projectId, null, node.path)).then((asked) => {
        if (asked.data) setChildren(asked.data.nodes)
      })
  }, [version])

  const folder = node.children !== null
  const drag = useRowDrag(projectId, node.path, folder)

  if (filtering && !keep.has(node.path)) return null

  const letter = mark(node.status)
  const showing = filtering ? true : folded

  function toggle(): void {
    if (!folder) return onOpen(node.path)
    setShutAt(collapsed)
    const next = !folded
    setOpen(next)
    if (next && children === null) {
      void ask(() => commands.projectTree(projectId, null, node.path)).then((asked) => {
        if (asked.data) setChildren(asked.data.nodes)
      })
    }
  }

  const kids = folder ? ordered(children ?? []) : []
  const visible = kids.slice(0, shown)
  const hidden = kids.length - visible.length

  return (
    <>
      <button
        className="row"
        data-ignored={node.status === 'ignored' || undefined}
        data-ctx="file"
        data-path={node.path}
        data-kind={folder ? 'folder' : 'file'}
        data-depth={depth}
        aria-current={node.path === current}
        aria-expanded={folder ? showing : undefined}
        data-drag-over={drag.over || undefined}
        style={{ paddingLeft: 8 + depth * 13 }}
        onClick={toggle}
        tabIndex={-1} // roving: `rove` (useTreeKeys.ts) hands exactly one row a 0
        draggable={drag.draggable}
        onDragStart={drag.onDragStart}
        onDragOver={drag.onDragOver}
        onDragLeave={drag.onDragLeave}
        onDrop={drag.onDrop}
        onDragEnd={drag.onDragEnd}
      >
        <span className="row__chev">{folder && <Chev open={showing} />}</span>
        <span className="row__ico">{folder ? <Dir /> : <Doc />}</span>
        <span className="row__n">{node.name}</span>
        {letter && <span className={`row__g row__g--${node.status}`}>{letter}</span>}
      </button>
      {folder &&
        showing &&
        visible.map((child) => (
          <Row
            key={child.path}
            projectId={projectId}
            node={child}
            version={version}
            depth={depth + 1}
            keep={keep}
            filtering={filtering}
            current={current}
            collapsed={collapsed}
            onOpen={onOpen}
          />
        ))}
      {folder && showing && hidden > 0 && (
        /* Reachable the same way Up/Down reach any other row — it is a plain
           sibling `.row` button, just one whose click reveals more instead
           of opening something. `data-kind="file"` keeps it out of the
           folder-only left/right handling. */
        <button
          className="row row--more"
          data-kind="file"
          data-depth={depth + 1}
          style={{ paddingLeft: 8 + (depth + 1) * 13 }}
          onClick={() => setShown((count) => count + MOST_SHOWN)}
          tabIndex={-1}
        >
          <span className="row__n">
            {t('workspace.row.more', { shown: formatNumber(visible.length), total: formatNumber(kids.length) })}
          </span>
        </button>
      )}
    </>
  )
}
