import { useState } from 'react'

import type { ErrorReportsPreview as Preview } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'

/* What the next error report would send, as the exact request body — so the
   person reads what leaves before it does, rather than a description of it. */
export function ErrorReportsPreview(): React.JSX.Element {
  const [preview, setPreview] = useState<Preview | null>(null)
  const [error, setError] = useState<string | null>(null)

  const show = (): void => {
    void ask(() => commands.errorsPreview()).then((answer) => {
      setPreview(answer.data)
      setError(answer.error)
    })
  }

  return (
    <div className="errprev">
      <button className="btn" onClick={preview ? () => setPreview(null) : show}>
        {preview ? t('workspace.errors.hide') : t('workspace.errors.show')}
      </button>
      {error && <p className="acc__note">{error}</p>}
      {preview && (
        <>
          <p className="acc__note">
            {preview.waiting === 0
              ? t('workspace.errors.noneWaiting')
              : t('workspace.errors.kept', { count: preview.waiting })}
          </p>
          {preview.waiting > 0 && <pre className="errprev__body">{preview.next}</pre>}
        </>
      )}
    </div>
  )
}
