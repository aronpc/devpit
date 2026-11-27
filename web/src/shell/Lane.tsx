import { t } from '../i18n'
import { Confirm } from './Confirm'
import { dueLabel, nearness } from './due'
import type { Lane as LaneData } from './board'
import type { Card, ColumnDeleted, Played, Step } from '../gen/bindings'
import { doingLabel } from './cardDoing'
import { InlineAdd } from './InlineAdd'
import { LaneMenu } from './LaneMenuView'
import { LaneStep } from './LaneStep'
import { money } from './chat'
import { closedLabel, named, stillBlocking } from './planEdit'
import { abandoned, committed } from './typing'
import { memo, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Icon } from './Icon'

const Clock = (): React.JSX.Element => (
  <Icon size="s"><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></Icon>
)

const Speech = (): React.JSX.Element => (
  <Icon size="s"><path d="M21 12a8 8 0 0 1-8 8H7l-4 3V12a8 8 0 0 1 8-8h2a8 8 0 0 1 8 8Z" /></Icon>
)

const Clip = (): React.JSX.Element => (
  <Icon size="s"><path d="M21 12.5 12.9 20.6a5 5 0 0 1-7.1-7.1l8.1-8.1a3.3 3.3 0 1 1 4.7 4.7l-8.1 8.1a1.7 1.7 0 0 1-2.4-2.4l7.5-7.4" /></Icon>
)

/* An epic's cards. */
const Stack = (): React.JSX.Element => (
  <Icon size="s"><rect x="4" y="9" width="16" height="11" rx="2" /><path d="M7 5h10" /></Icon>
)

/* Criteria met. */
const CheckBox = (): React.JSX.Element => (
  <Icon size="s"><rect x="3.5" y="3.5" width="17" height="17" rx="3" /><path d="m8 12 3 3 5-6" /></Icon>
)

const Spark = (): React.JSX.Element => (
  <Icon size="s">
    <path d="M12 3v3M5.6 5.6l2.1 2.1M3 12h3M18 12h3M16.3 7.7l2.1-2.1" />
    <rect x="7" y="12" width="10" height="9" rx="2" />
  </Icon>
)

/* Memoized, with callbacks that take the card: one card's activity or a run's
   line used to draw every tile on the board again. */
export const Tile = memo(function Tile({
  card,
  progress,
  stepName,
  onPlay,
  renaming,
  onRenamed,
  unread,
  onDoing,
}: {
  card: Card
  progress?: string
  /** The lane's step, for the button and for the question a step with no undo asks. */
  stepName?: string
  /** Present only when the tile can be played (`playable`); absent on the card in the air. */
  onPlay?: (cardId: string, confirmed: boolean) => Promise<Played | null>
  /** The title is being renamed in place, from the card's menu. */
  renaming?: boolean
  /** The new title, or null when the rename was abandoned. */
  onRenamed?: (card: Card, title: string | null) => void
  /** A finish on this card nobody has looked at yet. */
  unread?: boolean
  /** Goes where the dot points: the session waiting, or else the card. */
  onDoing?: (card: Card) => void
}): React.JSX.Element {
  const [asking, setAsking] = useState(false)
  const run = card.runs[0]
  const near = nearness(card.dueAt)
  const spent = money(card.costUsd ?? 0)
  const blocking = stillBlocking(card.blockedBy)
  const closed = closedLabel(card)
  const runLabel = stepName ? t('lane.runStep', { name: stepName }) : t('lane.runCardStep')
  const criteria = card.criteria ?? []
  const ticked = criteria.filter((one) => one.done).length
  return (
    <>
      {renaming ? (
        <TileRename title={card.title} onDone={(title) => onRenamed?.(card, title)} />
      ) : (
        <div className="tile__t">{card.title}</div>
      )}
      {/* Under the pointer rather than always there: the board is read far
          more often than it is played, and a row of triangles reads as a list
          of things waiting to be started. */}
      {onPlay && (
        <button
          className="tile__play"
          aria-label={runLabel}
          title={runLabel}
          onPointerDown={(event) => event.stopPropagation()}
          onClick={(event) => {
            event.stopPropagation()
            /* It used to open the card. It plays; a step with no undo asks first. */
            void onPlay(card.id, false).then((answer) => setAsking(Boolean(answer?.needsConfirming)))
          }}
        >
          <Icon size="s" filled>
            <path d="M8 5.5v13l11-6.5z" />
          </Icon>
        </button>
      )}
      {/* What the step is printing, as it prints it. One line, the last one:
          a tile is not a log, and the whole output is on the card. */}
      {progress && run?.state === 'running' && <div className="tile__log">{progress}</div>}
      {closed && <div className="tile__closed">{closed}</div>}
      {blocking.length > 0 && (
        <div className="tile__blocked" title={t('lane.blockedBy', { names: blocking.map(named).join(', ') })}>
          {t('lane.blockedBy', { names: blocking.map(named).join(', ') })}
        </div>
      )}
      <div className="tile__m">
        {card.number != null && <span className="tile__no">#{card.number}</span>}
        {card.activity && (
          <button
            className="tile__doing"
            data-doing={card.activity}
            data-unread={unread || undefined}
            aria-label={doingLabel(card.activity, Boolean(unread))}
            title={doingLabel(card.activity, Boolean(unread))}
            onPointerDown={(event) => event.stopPropagation()}
            onClick={(event) => {
              event.stopPropagation()
              onDoing?.(card)
            }}
          >
            {/* Colour alone does not carry a question to everyone. */}
            {card.activity === 'waiting' ? (
              <i className="card__ask" aria-hidden="true">
                ?
              </i>
            ) : (
              <i className="card__dot" aria-hidden="true" />
            )}
          </button>
        )}
        {run && (
          <span className={run.state === 'failed' || run.state === 'lost' ? 'tile__agent tile__agent--warn' : 'tile__agent'}>
            <Spark />
            {run.stepName}
          </span>
        )}
        {/* A colour and a phrase, never a badge shouting. The board is the
            person's own; a date that passed is information, not a telling-off. */}
        {near && (
          <span className="tile__due" data-near={near}>
            <Clock />
            {dueLabel(card.dueAt, undefined, card.dueTime)}
          </span>
        )}
        {card.progress && (
          <span
            className="tile__n tile__progress"
            data-done={card.progress.done === card.progress.total}
            title={t('lane.cardsDone', { count: card.progress.total, done: card.progress.done })}
          >
            <Stack />
            {card.progress.done}/{card.progress.total}
          </span>
        )}
        {criteria.length > 0 && (
          <span className="tile__n" title={t('lane.criteriaMet', { count: criteria.length, done: ticked })}>
            <CheckBox />
            {ticked}/{criteria.length}
          </span>
        )}
        {(card.planWaiting ?? 0) > 0 && (
          <span className="tile__plan" title={t('lane.planWaits', { count: card.planWaiting ?? 0 })}>
            {t('lane.plan', { count: card.planWaiting ?? 0 })}
          </span>
        )}
        {card.comments > 0 && (
          <span className="tile__n" title={t('lane.comments', { count: card.comments })}>
            <Speech />
            {card.comments}
          </span>
        )}
        {card.pinned > 0 && (
          <span className="tile__n" title={t('lane.files', { count: card.pinned })}>
            <Clip />
            {card.pinned}
          </span>
        )}
        {/* Only what was actually spent. `card_cost` sums the runs with a
            COALESCE, so a card that has never run answers 0.0 rather than
            nothing — and a chip reading $0.00 on every card is a number that
            says nothing taking the room of one that would. */}
        {spent && <span className="tile__time">{spent}</span>}
      </div>
      {asking && (
        <RunConfirm
          stepName={stepName}
          onClose={() => setAsking(false)}
          onConfirm={() => void onPlay?.(card.id, true).then(() => setAsking(false))}
        />
      )}
    </>
  )
})

/** The question a step with no undo asks before it runs, from the tile or its menu. */
export function RunConfirm({
  stepName,
  onClose,
  onConfirm,
}: {
  stepName?: string
  onClose: () => void
  onConfirm: () => void
}): React.JSX.Element {
  /* The portal is still inside the tile in React's tree: a press here must
     not start a drag or open the card. */
  return createPortal(
    <div onPointerDown={(event) => event.stopPropagation()} onKeyDown={(event) => event.stopPropagation()}>
      <Confirm
        title={stepName ? t('lane.runStepAsk', { name: stepName }) : t('lane.runThisStepAsk')}
        body={t('lane.noUndoBody')}
        danger={stepName ? t('lane.runStep', { name: stepName }) : t('lane.runIt')}
        onClose={onClose}
        onConfirm={onConfirm}
      />
    </div>,
    document.body,
  )
}

/** A title renamed in place. Settles once: Enter, Escape or leaving the field. */
function TileRename({ title, onDone }: { title: string; onDone: (title: string | null) => void }): React.JSX.Element {
  const done = useRef(false)
  const finish = (next: string | null): void => {
    if (done.current) return
    done.current = true
    onDone(next)
  }
  return (
    <input
      className="tile__rename"
      autoFocus
      defaultValue={title}
      aria-label={t('lane.cardTitle')}
      onPointerDown={(event) => event.stopPropagation()}
      onKeyDown={(event) => {
        event.stopPropagation()
        if (committed(event)) {
          event.preventDefault()
          finish(event.currentTarget.value.trim() || null)
        }
        if (abandoned(event)) {
          event.preventDefault()
          finish(null)
        }
      }}
      onBlur={(event) => finish(event.currentTarget.value.trim() || null)}
    />
  )
}

/* The lane's own head and foot. The delete confirm lives here because this is
   what knows how many cards would go with it. */
export function LaneHead({
  lane,
  steps,
  onRename,
  onPickStep,
  onCreateStep,
  onUpdateStep,
  onRemoveStep,
  others,
  onFlow,
  onGrab,
  onAddCard,
  onShift,
  onDelete,
  onRole,
}: {
  lane: LaneData
  steps: readonly Step[]
  onRename: (name: string) => void
  onRole?: (role: string | null) => void
  onAddCard: () => void
  onShift: (by: -1 | 1) => void
  onDelete: (moveTo: string | null) => Promise<ColumnDeleted | null>
  onPickStep: (stepId: string | null) => void
  onCreateStep: (kind: string, name: string, config: string, irreversible: boolean) => void
  onUpdateStep: (stepId: string, name: string, config: string, irreversible: boolean) => void
  onRemoveStep: (stepId: string) => void
  /** The other lanes, for the one a pass sends a card to. */
  others: readonly { id: string; name: string }[]
  onFlow: (onPass: string | null, autonomy: string) => void
  /** Starts a column drag. The head is the handle: the list below it is
      already a drop target for cards, and one surface cannot be both. */
  onGrab?: (event: React.PointerEvent) => void
}): React.JSX.Element {
  /* Not editable on a single click: the head is the drag handle, and a press
     that meant to carry the lane kept landing in the name. */
  const [renaming, setRenaming] = useState(false)
  const [menu, setMenu] = useState(false)
  const label = useRef<HTMLSpanElement>(null)

  useEffect(() => {
    const el = label.current
    if (!renaming || !el) return
    el.focus()
    const range = document.createRange()
    range.selectNodeContents(el)
    window.getSelection()?.removeAllRanges()
    window.getSelection()?.addRange(range)
  }, [renaming])

  return (
    <div
      className="blane__top"
      onPointerDown={onGrab}
      onContextMenu={(event) => {
        event.preventDefault()
        event.stopPropagation()
        setMenu(true)
      }}
    >
      <span
        ref={label}
        className="blane__label"
        contentEditable={renaming}
        suppressContentEditableWarning
        role="textbox"
        aria-label={t('lane.nameOf', { name: lane.column.name })}
        aria-readonly={!renaming}
        onDoubleClick={() => setRenaming(true)}
        onKeyDown={(event) => {
          if (committed(event)) {
            event.preventDefault()
            event.currentTarget.blur()
          }
          if (abandoned(event)) {
            event.currentTarget.textContent = lane.column.name
            event.currentTarget.blur()
          }
        }}
        onBlur={(event) => {
          setRenaming(false)
          const name = event.currentTarget.textContent?.trim()
          if (name && name !== lane.column.name) onRename(name)
          else event.currentTarget.textContent = lane.column.name
        }}
      >
        {lane.column.name}
      </span>
      <span className="blane__n">{lane.cards.length}</span>
      <LaneStep
        step={lane.column.step}
        steps={steps}
        lanes={others}
        onPass={lane.column.onPass}
        autonomy={lane.column.autonomy}
        onPick={onPickStep}
        onCreate={onCreateStep}
        onUpdate={onUpdateStep}
        onRemove={onRemoveStep}
        onFlow={onFlow}
      />
      <LaneMenu
        name={lane.column.name}
        others={others}
        open={menu}
        onOpen={setMenu}
        onRename={() => setRenaming(true)}
        onAddCard={onAddCard}
        onShift={onShift}
        onDelete={onDelete}
        role={lane.column.role}
        roleChosen={lane.column.roleChosen}
        onRole={onRole}
      />
    </div>
  )
}

export function LaneFoot({
  adding,
  onAdding,
  onAddCard,
}: {
  /** Held by the board: the lane menu's Add card opens the same field. */
  adding: boolean
  onAdding: (open: boolean) => void
  onAddCard: (title: string) => void
}): React.JSX.Element {
  return (
    <>
      {adding ? (
        <InlineAdd
          className="tile__new"
          label={t('lane.newCardTitle')}
          placeholder={t('lane.cardTitle')}
          onAdd={onAddCard}
          onDone={() => onAdding(false)}
        />
      ) : (
        <button className="tile__add" onClick={() => onAdding(true)}>
          {t('lane.addCard')}
        </button>
      )}
      <div className="blane__fill" />
    </>
  )
}
