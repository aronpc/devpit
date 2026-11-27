import { TRAY } from './Empty'
import { Icon } from './Icon'

/*
 * What every view of the right panel opens with, and what it says when it
 * has nothing: one title row, one empty state. Six views had four headers
 * and two kinds of empty, and the person learned each one separately.
 */

export function PanelHead({
  title,
  meta,
  children,
}: {
  title: string
  /** A count or a state, beside the title. */
  meta?: React.ReactNode
  /** The view's actions, as `PanelAct`s. */
  children?: React.ReactNode
}): React.JSX.Element {
  return (
    <div className="phead">
      <span className="phead__t">{title}</span>
      {meta !== undefined && meta !== null && meta !== false && <span className="phead__m">{meta}</span>}
      <span className="phead__acts">{children}</span>
    </div>
  )
}

/** An action in a view's title row: an icon, named in its tooltip. */
export function PanelAct({
  label,
  onClick,
  active,
  children,
}: {
  label: string
  onClick: () => void
  active?: boolean
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <button className="phead__act" onClick={onClick} aria-label={label} data-tip={label} aria-pressed={active}>
      {children}
    </button>
  )
}

/** A view with nothing in it: what would be here, and how it gets here. */
export function PanelEmpty({
  title,
  hint,
  icon,
  children,
}: {
  title: string
  /** Its glyph, on the 24 grid; a tray unless said. */
  icon?: React.ReactNode
  hint?: React.ReactNode
  /** One action that fills it, when there is one. */
  children?: React.ReactNode
}): React.JSX.Element {
  return (
    <div className="pempty">
      <Icon size="l">{icon ?? TRAY}</Icon>
      <span className="pempty__t">{title}</span>
      {hint && <span className="pempty__d">{hint}</span>}
      {children}
    </div>
  )
}
