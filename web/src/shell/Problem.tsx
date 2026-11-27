import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { Icon } from './Icon'
import { PROBLEM } from './problems'
import { Toast } from './Toast'

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
    <Toast
      role="alert"
      tone="danger"
      icon={<Icon size="m"><circle cx="12" cy="12" r="9" /><path d="M12 8v5M12 16.5v.01" /></Icon>}
      action={{ label: t('workspace.problem.dismiss'), onClick: () => setWhy(null) }}
    >
      {why}
    </Toast>
  )
}
