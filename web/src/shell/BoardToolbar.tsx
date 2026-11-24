import { useEffect, useState } from 'react'

import { t } from '../i18n'
import type { Lane } from './board'
import { ArchivedPane, UndoArchive } from './BoardShelf'
import { InlineAdd } from './InlineAdd'
import { ask, commands } from './live'
import { RunsPane } from './RunsPane'

/*
 * The board's toolbar: what to show, and what sits beside the lanes.
 *
 * These used to be buttons after the last lane, at the end of a horizontal
 * scroll — found by scrolling past all the work to look for them.
 */

export function NewColumn({ onAdd }: { onAdd: (name: string) => void }): React.JSX.Element {
  const [open, setOpen] = useState(false)
  return open ? (
    <InlineAdd
      className="blane__newin"
      label={t('board.newColumnName')}
      placeholder={t('board.columnName')}
      onAdd={onAdd}
      onDone={() => setOpen(false)}
    />
  ) : (
    <button className="btn" onClick={() => setOpen(true)}>
      {t('board.addColumn')}
    </button>
  )
}

export function BoardToolbar({
  projectId,
  lanes,
  query,
  onQuery,
  archived,
  onUndone,
  onOpenCard,
  onChanged,
  onAddColumn,
}: {
  projectId: string | null
  lanes: readonly Lane[]
  query: string
  onQuery: (query: string) => void
  /** The card just archived, while Undo is still worth offering. */
  archived: string | null
  onUndone: () => void
  onOpenCard: (cardId: string) => void
  onChanged: () => void
  onAddColumn: (name: string) => void
}): React.JSX.Element | null {
  const [showing, setShowing] = useState<'runs' | 'archived' | null>(null)
  const [count, setCount] = useState<number | null>(null)
  /* Counted again when a card is archived, restored or deleted from the list. */
  const [counted, setCounted] = useState(0)

  useEffect(() => {
    if (!projectId) return
    void ask(() => commands.boardArchived(projectId)).then((answer) => {
      if (answer.data) setCount(answer.data.cards.length)
    })
  }, [projectId, archived, counted])

  if (!projectId) return null
  const changed = (): void => {
    setCounted((was) => was + 1)
    onChanged()
  }

  return (
    <div className="btools">
      <input
        className="btools__filter"
        type="search"
        aria-label={t('board.filterCards')}
        placeholder={t('board.filterCards')}
        value={query}
        onChange={(event) => onQuery(event.target.value)}
      />
      <span className="btools__gap" />
      <button className="btn" onClick={() => setShowing('runs')}>
        {t('board.runs')}
      </button>
      <button className="btn" onClick={() => setShowing('archived')}>
        {count === null ? t('board.archived') : t('board.archivedCount', { count })}
      </button>
      <NewColumn onAdd={onAddColumn} />

      {showing === 'runs' && (
        <RunsPane
          projectId={projectId}
          lanes={lanes}
          onClose={() => setShowing(null)}
          onOpenCard={(cardId) => {
            setShowing(null)
            onOpenCard(cardId)
          }}
        />
      )}
      {showing === 'archived' && (
        <ArchivedPane projectId={projectId} onClose={() => setShowing(null)} onChanged={changed} />
      )}
      {archived && <UndoArchive projectId={projectId} cardId={archived} onRestored={changed} onGone={onUndone} />}
    </div>
  )
}
