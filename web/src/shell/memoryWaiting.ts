import { useEffect, useState } from 'react'

import { ask, commands } from './live'
import { onCarried } from './window'

/*
 * How many memory proposals wait for the person, project by project: a count
 * on the project in the rail and on its "Project agent", never a banner.
 */

export function waitingByProject(proposals: readonly { projectId: string }[]): ReadonlyMap<string, number> {
  const counts = new Map<string, number>()
  for (const one of proposals) counts.set(one.projectId, (counts.get(one.projectId) ?? 0) + 1)
  return counts
}

export function useMemoryWaiting(): ReadonlyMap<string, number> {
  const [counts, setCounts] = useState<ReadonlyMap<string, number>>(new Map())
  useEffect(() => {
    const read = (): void => {
      void ask(() => commands.lessonsProposed()).then((said) => {
        if (said.data) setCounts(waitingByProject(said.data.proposals))
      })
    }
    read()
    return onCarried<string>('lessons:proposed', read)
  }, [])
  return counts
}
