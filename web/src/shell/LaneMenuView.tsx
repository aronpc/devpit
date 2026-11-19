import { useCallback, useRef, useState } from 'react'
import { createPortal } from 'react-dom'

import type { ColumnDeleted } from '../gen/bindings'
import { t } from '../i18n'
import { useAway } from './away'
import { Confirm } from './Confirm'
import { laneMenu } from './laneMenu'

/** What the dialog says when a lane still holds cards: the backend's count, and a choice. */
export const moveTitle = (inTheWay: number, lane: string): string => t('lane.moveOut', { count: inTheWay, lane })

/* Presses inside stay here: the head around this is the lane's drag handle. */
const stay = (event: React.SyntheticEvent): void => event.stopPropagation()

export function LaneMenu({
  name,
  others,
  open,
  onOpen,
  onRename,
  onAddCard,
  onShift,
  onDelete,
  role = null,
  roleChosen = false,
  onRole,
}: {
  name: string
  /** What the lane is for, and whether the person said so. */
  role?: string | null
  roleChosen?: boolean
  onRole?: (role: string | null) => void
  /** The lanes its cards can go to. */
  others: readonly { id: string; name: string }[]
  /** Held by the head, which also opens it on a right-click. */
  open: boolean
  onOpen: (open: boolean) => void
  onRename: () => void
  onAddCard: () => void
  onShift: (by: -1 | 1) => void
  onDelete: (moveTo: string | null) => Promise<ColumnDeleted | null>
}): React.JSX.Element {
  const box = useRef<HTMLDivElement>(null)
  useAway(box, useCallback(() => onOpen(false), [onOpen]), open)
  const [asking, setAsking] = useState<'confirm' | { readonly inTheWay: number } | null>(null)
  const [to, setTo] = useState('')
  const target = to || others[0]?.id || ''
  const entries = laneMenu(
    {
      rename: onRename,
      addCard: onAddCard,
      shift: onShift,
      remove: () => setAsking('confirm'),
      role: onRole,
    },
    { role, chosen: roleChosen },
  )

  return (
    <div className="ctl" ref={box} onPointerDown={stay}>
      <button className="sq26" aria-label={t('lane.actions', { name })} title={t('lane.actions', { name })} aria-expanded={open} onClick={() => onOpen(!open)}>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor" stroke="none">
          <circle cx="5" cy="12" r="1.8" />
          <circle cx="12" cy="12" r="1.8" />
          <circle cx="19" cy="12" r="1.8" />
        </svg>
      </button>
      {open && (
        <div className="ctlmenu ctlmenu--down" role="menu" aria-label={t('lane.actions', { name })}>
          {entries.map((entry, index) =>
            entry.rule ? (
              <div key={index} className="newmenu__rule" />
            ) : (
              <button
                key={index}
                className="ctlmenu__i"
                role="menuitem"
                data-danger={entry.bad}
                onClick={() => {
                  onOpen(false)
                  entry.run?.(name)
                }}
              >
                <span className="ctlmenu__b">
                  <span className="ctlmenu__n">{entry.label}</span>
                </span>
              </button>
            ),
          )}
        </div>
      )}

      {/* On the body: `.ask` fills its positioned ancestor, and a lane is one.
          Still inside the head in React's tree, so presses stop at the wrapper. */}
      {asking === 'confirm' &&
        createPortal(
          <div onPointerDown={stay} onKeyDown={stay}>
            <Confirm
              title={t('lane.deleteAsk', { name })}
              body={t('lane.deleteBody')}
              onClose={() => setAsking(null)}
              onConfirm={() => {
                /* Asked without a destination first: the backend counts what is
                   in the way, and that count is what the next question says. */
                void onDelete(null).then((answer) =>
                  setAsking(answer && !answer.deleted && answer.cardsInTheWay > 0 ? { inTheWay: answer.cardsInTheWay } : null),
                )
              }}
            />
          </div>,
          document.body,
        )}
      {asking &&
        asking !== 'confirm' &&
        createPortal(
          <div
            className="ask"
            data-open="true"
            onPointerDown={stay}
            onKeyDown={stay}
            onClick={(event) => event.target === event.currentTarget && setAsking(null)}
          >
            <div className="ask__box" role="dialog" aria-modal="true" aria-label={t('lane.deleteLabel', { name })}>
              <h2 className="ask__t">{moveTitle(asking.inTheWay, name)}</h2>
              {others.length > 0 ? (
                <label className="fld">
                  <span className="fld__l">{t('lane.moveThemTo')}</span>
                  <select className="fld__b" value={target} onChange={(event) => setTo(event.target.value)}>
                    {others.map((other) => (
                      <option key={other.id} value={other.id}>
                        {other.name}
                      </option>
                    ))}
                  </select>
                </label>
              ) : (
                <p className="ask__d">{t('lane.onlyLane')}</p>
              )}
              <div className="ask__row">
                <button className="btn" onClick={() => setAsking(null)}>
                  {t('lane.cancel')}
                </button>
                <button
                  className="btn btn--danger"
                  disabled={!target}
                  onClick={() => void onDelete(target).then(() => setAsking(null))}
                >
                  {t('lane.moveAndDelete')}
                </button>
              </div>
            </div>
          </div>,
          document.body,
        )}
    </div>
  )
}
