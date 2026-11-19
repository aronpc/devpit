import { useRef, useState } from 'react'

import { createPortal } from 'react-dom'

import type { Step } from '../gen/bindings'
import { t } from '../i18n'
import { Confirm } from './Confirm'
import { StepNew } from './StepNew'
import { useAway } from './away'
import { stepFields } from './stepConfig'

/* Presses inside stay here: the lane head around this is its drag handle. */
const stay = (event: React.SyntheticEvent): void => event.stopPropagation()

/*
 * What a lane runs when a card lands in it.
 *
 * The rule the whole product turns on, and until now there was no way to set
 * it: `column_set_step` and `step_create` both existed in the contract and
 * nothing anywhere called them, so every lane ran nothing forever.
 *
 * A step marked irreversible is not fired by a drag — the backend already
 * refuses that, and the badge here says so, because a person choosing a deploy
 * step should learn the rule when they choose it and not when it surprises
 * them.
 */

export function LaneStep({
  step,
  steps,
  lanes,
  onPass,
  autonomy,
  onPick,
  onCreate,
  onUpdate,
  onRemove,
  onFlow,
}: {
  step: Step | null
  steps: readonly Step[]
  /** Every other lane on this board, for the one a pass sends a card to. */
  lanes: readonly { id: string; name: string }[]
  onPass: string | null
  autonomy: string
  onPick: (stepId: string | null) => void
  onCreate: (kind: string, name: string, config: string, irreversible: boolean) => void
  onUpdate: (stepId: string, name: string, config: string, irreversible: boolean) => void
  onRemove: (stepId: string) => void
  onFlow: (onPass: string | null, autonomy: string) => void
}): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const [making, setMaking] = useState(false)
  const [editing, setEditing] = useState(false)
  const [removing, setRemoving] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  useAway(box, () => setOpen(false), open)
  // Said on the lane and not only when the form opens: a step nothing can read
  // is a lane that will fail when a card lands on it.
  const unreadable = step !== null && stepFields(step.kind, step.config) === null

  return (
    <div className="lstep" ref={box}>
      <button
        className="lstep__b"
        aria-expanded={open}
        title={step ? t('step.runs', { name: step.name }) : t('step.runsNothing')}
        onClick={() => setOpen((was) => !was)}
      >
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 3v3M5.6 5.6l2.1 2.1M3 12h3M18 12h3M16.3 7.7l2.1-2.1" />
          <rect x="7" y="12" width="10" height="9" rx="2" />
        </svg>
        {step?.name ?? t('step.noStep')}
        {/* Said on the lane, not hidden in its menu: a lane that moves cards
            without being asked should look different from one that does not. */}
        {step && autonomy === 'auto' && <span className="lstep__auto">{t('step.auto')}</span>}
        {unreadable && <span className="lstep__warn">{t('step.cannotBeRead')}</span>}
      </button>

      {open && !making && !editing && (
        <div className="lstep__pop" role="menu">
          <p className="lstep__t">{t('step.runsWhen')}</p>
          <button
            className="apps__opt"
            role="menuitem"
            aria-checked={step === null}
            onClick={() => {
              onPick(null)
              setOpen(false)
            }}
          >
            {t('step.nothing')}
          </button>
          {steps.map((one) => (
            <button
              className="apps__opt"
              key={one.id}
              role="menuitem"
              aria-checked={step?.id === one.id}
              onClick={() => {
                onPick(one.id)
                setOpen(false)
              }}
            >
              <span>{one.name}</span>
              {one.irreversible && <span className="lstep__warn">{t('step.noUndo')}</span>}
            </button>
          ))}
          <div className="newmenu__rule" />
          <button className="apps__opt" role="menuitem" onClick={() => setMaking(true)}>
            {t('step.newStep')}
          </button>

          {/* Only once the lane runs something. Where a pass goes and how much
              the lane decides are answers about a step, and a lane with no
              step has nothing to decide about. */}
          {step && (
            <>
              <div className="newmenu__rule" />
              {unreadable && <p className="lstep__t">{t('step.settingsUnreadable')}</p>}
              <button className="apps__opt" role="menuitem" onClick={() => setEditing(true)}>
                {t('step.edit', { name: step.name })}
              </button>
              <button className="apps__opt" role="menuitem" onClick={() => setRemoving(true)}>
                <span>{t('step.delete', { name: step.name })}</span>
                <span className="lstep__warn">{t('step.noUndo')}</span>
              </button>

              <div className="newmenu__rule" />
              <p className="lstep__t">{t('step.whenPasses')}</p>
              <button
                className="apps__opt"
                role="menuitem"
                aria-checked={!onPass}
                onClick={() => onFlow(null, 'manual')}
              >
                {t('step.stayHere')}
              </button>
              {lanes.map((one) => (
                <button
                  className="apps__opt"
                  key={one.id}
                  role="menuitem"
                  aria-checked={onPass === one.id}
                  onClick={() => onFlow(one.id, autonomy === 'manual' ? 'ask' : autonomy)}
                >
                  {t('step.moveTo', { name: one.name })}
                </button>
              ))}

              {onPass && (
                <>
                  <div className="newmenu__rule" />
                  <p className="lstep__t">{t('step.andMoves')}</p>
                  <button
                    className="apps__opt"
                    role="menuitem"
                    aria-checked={autonomy === 'ask'}
                    onClick={() => onFlow(onPass, 'ask')}
                  >
                    <span>{t('step.whenYouSay')}</span>
                  </button>
                  <button
                    className="apps__opt"
                    role="menuitem"
                    aria-checked={autonomy === 'auto'}
                    onClick={() => onFlow(onPass, 'auto')}
                  >
                    <span>{t('step.onItsOwn')}</span>
                    <span className="lstep__warn">{t('step.noPrompt')}</span>
                  </button>
                </>
              )}
            </>
          )}
        </div>
      )}

      {open && making && (
        <StepNew
          onCancel={() => setMaking(false)}
          onDone={(kind, name, config, irreversible) => {
            onCreate(kind, name, config, irreversible)
            setMaking(false)
            setOpen(false)
          }}
        />
      )}

      {open && editing && step && (
        <StepNew
          step={step}
          onCancel={() => setEditing(false)}
          onDone={(_kind, name, config, irreversible) => {
            onUpdate(step.id, name, config, irreversible)
            setEditing(false)
            setOpen(false)
          }}
        />
      )}

      {/* On the body, like the lane's own delete: `.ask` fills its positioned
          ancestor, and this popover is one. */}
      {removing &&
        step &&
        createPortal(
          <div onPointerDown={stay} onKeyDown={stay}>
            <Confirm
              title={t('step.deleteAsk', { name: step.name })}
              body={t('step.deleteBody')}
              onClose={() => setRemoving(false)}
              onConfirm={() => {
                onRemove(step.id)
                setRemoving(false)
                setOpen(false)
              }}
            />
          </div>,
          document.body,
        )}
    </div>
  )
}
