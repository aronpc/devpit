import { useState } from 'react'

import type { LiveSession } from '../gen/bindings'
import { ComebackBriefing } from './ComebackBriefing'
import { ForwardedDrafts } from './ForwardedDrafts'
import { refreshSessions, useLiveSessions } from './liveStatus'
import { ProjectProposals } from './ProjectProposals'
import { ReplyDrafts } from './ReplyDrafts'
import { SHOW_PANEL } from './RightPanel'
import { SessionTerminal } from './SessionTerminal'
import { StopsAsked } from './StopsAsked'
import { useShell } from './useShell'
import { WaitingPrompts } from './WaitingPrompts'

/*
 * What an orchestrator's chat adds for the sessions it works with: their
 * count in its corner, which opens the Sessions panel, and — above the
 * composer — every question one of them is stopped on, answered from here or
 * in its own terminal opened over the chat, every reply the orchestrator
 * drafted for the person to send as theirs, the stops it asked for, and the
 * requests other orchestrators forwarded to it.
 */

export function SessionsChip({ profileId }: { profileId: string }): React.JSX.Element {
  const live = useLiveSessions(profileId)
  const waiting = live.filter((one) => one.waiting).length
  return (
    <button className="pcorner__sess" onClick={() => window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: 'sessions' }))} title="The sessions this orchestrator works with">
      {live.length} {live.length === 1 ? 'session' : 'sessions'}
      {waiting > 0 && <b> · {waiting} waiting</b>}
    </button>
  )
}

export function SessionsWaiting({ profileId }: { profileId: string }): React.JSX.Element {
  const { setProject, openPane, project, active } = useShell()
  const live = useLiveSessions(profileId)
  const [terminal, setTerminal] = useState<LiveSession | null>(null)
  return (
    <>
      <WaitingPrompts profileId={profileId} sessions={live} onAnswered={() => refreshSessions(profileId)} onOpen={setTerminal} />
      <ReplyDrafts profileId={profileId} sessions={live} onDone={() => refreshSessions(profileId)} />
      <StopsAsked profileId={profileId} sessions={live} onDone={() => refreshSessions(profileId)} />
      <ProjectProposals />
      {project?.orchestrator && <ForwardedDrafts projectId={project.id} conversation={active?.id ?? null} />}
      {project?.orchestrator && <ComebackBriefing projectId={project.id} />}
      {terminal && (
        <SessionTerminal
          session={terminal}
          profileId={profileId}
          onClose={() => setTerminal(null)}
          onGo={() => {
            setTerminal(null)
            if (terminal.projectId) setProject(terminal.projectId)
            if (terminal.pane) openPane(terminal.pane.paneId)
          }}
        />
      )}
    </>
  )
}
