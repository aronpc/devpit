import { useForwarded } from './forwardedRequests'

/*
 * The mark on an orchestrator in the rail when other orchestrators forwarded
 * it requests: how many wait, and nothing else. Their words are in its chat.
 */

export function ForwardedMark({ projectId }: { projectId: string }): React.JSX.Element | null {
  const { requests } = useForwarded(projectId)
  if (requests.length === 0) return null
  return (
    <span className="rail__memo" role="status" aria-label={`${requests.length} forwarded ${requests.length === 1 ? 'request waits' : 'requests wait'}`}>
      {requests.length}
    </span>
  )
}
