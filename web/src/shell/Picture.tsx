import { useState } from 'react'

import { bytes } from './disk'
import { t } from '../i18n'

/*
 * A picture a file read handed back, drawn inline.
 *
 * One component for both places a file opens — a tab and the workspace
 * browser — because what goes wrong is the same in both: a data URL the
 * webview cannot decode draws nothing, and an empty pane says nothing about
 * why. A picture that fails says so instead.
 */

export function Picture({ src, name, size }: { src: string; name: string; size: number | null }): React.JSX.Element {
  const [broken, setBroken] = useState(false)
  /* Fitted to the pane, or at its own size: a click switches. */
  const [actual, setActual] = useState(false)
  const [drawn, setDrawn] = useState<{ width: number; height: number } | null>(null)

  if (broken) {
    return (
      <div className="exempty">
        <span className="exempty__t">{t('workspace.picture.cannotDraw', { name })}</span>
        <span className="exempty__d">{bytes(size) ?? t('workspace.picture.empty')}</span>
      </div>
    )
  }
  return (
    <div className="media">
      <img
        className="media__img"
        data-actual={actual || undefined}
        src={src}
        alt={name}
        title={actual ? t('workspace.picture.fit') : t('workspace.picture.actual')}
        onClick={() => setActual((was) => !was)}
        onLoad={(event) => setDrawn({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight })}
        onError={() => setBroken(true)}
      />
      <div className="media__what">{[drawn && `${drawn.width} × ${drawn.height}`, bytes(size)].filter(Boolean).join(' · ')}</div>
    </div>
  )
}
