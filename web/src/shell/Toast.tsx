import { createPortal } from 'react-dom'

/*
 * A short word at the foot of the window: what just happened, and the one
 * thing to do about it. Every toast lands in one stack, so two never sit on
 * each other; each arrives with a short rise and leaves at once.
 */

export type Tone = 'info' | 'danger'

let stack: HTMLElement | null = null

/** The one stack every toast is drawn into, made on first use. */
function stackRoot(): HTMLElement {
  if (!stack || !stack.isConnected) {
    stack = document.createElement('div')
    stack.className = 'toasts'
    document.body.appendChild(stack)
  }
  return stack
}

export function Toast({
  tone = 'info',
  role = 'status',
  icon,
  action,
  children,
}: {
  tone?: Tone
  role?: 'status' | 'alert'
  icon?: React.ReactNode
  action?: { label: string; onClick: () => void }
  children: React.ReactNode
}): React.JSX.Element {
  return createPortal(
    <div className="toast" data-tone={tone} role={role}>
      {icon && <span className="toast__ico">{icon}</span>}
      <span className="toast__t">{children}</span>
      {action && (
        <button className="toast__act" onClick={action.onClick}>
          {action.label}
        </button>
      )}
    </div>,
    stackRoot(),
  )
}
