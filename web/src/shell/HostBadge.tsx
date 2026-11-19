import { useEffect, useState } from 'react'

import type { HostOfTab } from '../gen/bindings'
import { ask, commands } from './live'

/*
 * Which host a tab's terminals run on, its environment and whether devpit's
 * connection to it is up. Production reads PROD, in red: a tab there is never
 * mistaken for a local one. A tab on this computer shows nothing.
 */

/* `shape` is the tab's panes as it draws them: a join or a split changes what
   runs there, and the badge asks again. */
export function HostBadge({ projectId, tabId, shape = '', every = 10_000 }: { projectId: string; tabId: string; shape?: string; every?: number }): React.JSX.Element | null {
  const [host, setHost] = useState<{ key: string; host: HostOfTab | null } | null>(null)
  // What is shown belongs to the tab, not to its shape: a split or a join
  // asks again, and shows what it had until the answer.
  const key = `${projectId}/${tabId}`

  useEffect(() => {
    let gone = false
    let timer: ReturnType<typeof setTimeout> | undefined
    const look = (): void => {
      void ask(() => commands.tabHost(projectId, tabId)).then((answer) => {
        if (gone) return
        /* A failed look keeps what was shown, PROD above all, and looks again. */
        if (answer.error) {
          timer = setTimeout(look, every)
          return
        }
        const found = answer.data?.host ?? null
        setHost({ key, host: found })
        /* Asked again only for a tab on a host: its connection can drop. */
        if (found) timer = setTimeout(look, every)
      })
    }
    look()
    return () => {
      gone = true
      clearTimeout(timer)
    }
  }, [projectId, tabId, key, shape, every])

  const on = host?.key === key ? host.host : null
  if (!on) return null
  const down = on.connected === false
  const env = on.environment === 'production' ? 'PROD' : on.environment
  const said = `${on.name}, ${on.environment}${down ? ', disconnected' : on.connected ? ', connected' : ''}`
  return (
    <span className="tab__host" data-env={on.environment} data-down={down ? 'true' : undefined} title={said} aria-label={said}>
      <b>{env}</b>
      <span className="tab__hostn">{on.name}</span>
    </span>
  )
}
