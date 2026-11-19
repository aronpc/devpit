import { useState } from 'react'

import { abandoned, committed } from './typing'
import { t } from '../i18n'

/* One dialog for anything that needs a name before it can happen. Shaped like
   `Confirm`, because the two are the same act with and without a text field,
   and two different-looking modals for that would read as two different
   kinds of decision. */
export function AskName({
  title,
  placeholder,
  action,
  onClose,
  onName,
}: {
  title: string
  placeholder: string
  action: string
  onClose: () => void
  onName: (name: string) => void
}): React.JSX.Element {
  const [name, setName] = useState('')
  const ready = name.trim().length > 0

  /* A name with a separator in it would reach outside the folder that was
     right-clicked. The backend refuses it too — this is the half that can say
     so before the round trip. */
  const usable = ready && !name.includes('/')

  return (
    <div
      className="ask"
      data-open="true"
      onClick={(event) => event.target === event.currentTarget && onClose()}
    >
      <div className="ask__box" role="dialog" aria-modal="true">
        <h2 className="ask__t">{title}</h2>
        <input
          className="git__msg"
          style={{ minHeight: 'auto', height: 32 }}
          placeholder={placeholder}
          value={name}
          autoFocus
          aria-label={title}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => {
            if (committed(event) && usable) onName(name.trim())
            if (abandoned(event)) onClose()
          }}
        />
        {ready && !usable && (
          <p className="ask__d">{t('workspace.askName.slash')}</p>
        )}
        <div className="ask__row">
          <button className="btn" onClick={onClose}>
            {t('workspace.askName.cancel')}
          </button>
          <button className="btn btn--go" disabled={!usable} onClick={() => onName(name.trim())}>
            {action}
          </button>
        </div>
      </div>
    </div>
  )
}
