import { useCallback, useEffect, useState } from 'react'

import type { ForwardedRequest } from '../gen/bindings'
import { ask, commands } from './live'
import { onCarried } from './window'

/*
 * What other orchestrators forwarded to one project's orchestrator, kept by
 * the app and read again whenever it says they changed. Reading starts
 * nothing: a request waits for the person.
 */

export function useForwarded(projectId: string | null): { requests: readonly ForwardedRequest[]; reload: () => void } {
  const [requests, setRequests] = useState<readonly ForwardedRequest[]>([])
  const reload = useCallback(() => {
    if (!projectId) return
    void ask(() => commands.forwardedList(projectId)).then((said) => said.data && setRequests(said.data.requests))
  }, [projectId])
  useEffect(reload, [reload])
  useEffect(() => onCarried<string>('orchestrator:forwarded', (id) => id === projectId && reload()), [projectId, reload])
  return { requests, reload }
}
