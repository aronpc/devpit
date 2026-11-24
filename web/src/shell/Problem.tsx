import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { PROBLEM } from './problems'

/* The last refusal `report` heard, at the foot of the window until dismissed
   or replaced. Not on a timer: a reason that leaves before it is read was
   not said. */
export function Problem(): React.JSX.Element | null {
  const [why, setWhy] = useState<string | null>(null)

  useEffect(() => {
    const heard = (event: Event): void => setWhy((event as CustomEvent<string>).detail)
    window.addEventListener(PROBLEM, heard)
    return () => window.removeEventListener(PROBLEM, heard)
  }, [])

  if (!why) return null
  return (
    <div className="undo" role="alert">
      {why}
      <button className="undo__b" onClick={() => setWhy(null)}>
        {t('workspace.problem.dismiss')}
      </button>
    </div>
  )
}
