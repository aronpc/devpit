import { useEffect } from 'react'

import { t } from '../i18n'
import { abandoned } from './typing'

/* One dialog for anything that cannot be undone. The body has to name what
   is lost — "are you sure?" tells the reader nothing they did not know. */
export function Confirm({
  title,
  body,
  danger,
  onClose,
  onConfirm,
}: {
  title: string
  body: React.ReactNode
  danger?: string
  onClose: () => void
  onConfirm: () => void
}): React.JSX.Element {
  /* Escape is Cancel. Over an open card, the card leaves it to this. */
  useEffect(() => {
    const key = (event: KeyboardEvent): void => {
      if (abandoned(event)) onClose()
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }, [onClose])

  return (
    <div
      className="ask"
      data-open="true"
      onClick={(event) => event.target === event.currentTarget && onClose()}
    >
      <div className="ask__box" role="dialog" aria-modal="true">
        <h2 className="ask__t">{title}</h2>
        <p className="ask__d">{body}</p>
        <div className="ask__row">
          <button className="btn" onClick={onClose}>
            {t('workspace.confirm.cancel')}
          </button>
          <button className="btn btn--danger" onClick={onConfirm}>
            {danger ?? t('workspace.confirm.delete')}
          </button>
        </div>
      </div>
    </div>
  )
}
