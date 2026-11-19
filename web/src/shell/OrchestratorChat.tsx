import { useState } from 'react'

import { t } from '../i18n'
import type { LiveSession } from '../gen/bindings'
import { ComebackBriefing } from './ComebackBriefing'
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
 * drafted for the person to send as theirs, and the stops it asked for.
 */

export function SessionsChip({ profileId }: { profileId: string }): React.JSX.Element {
  const live = useLiveSessions(profileId)
  const waiting = live.filter((one) => one.waiting).length
  return (
    <button className="pcorner__sess" onClick={() => window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: 'sessions' }))} title={t('workspace.orchestratorChat.sessionsTitle')}>
      {t('workspace.orchestratorChat.sessions', { count: live.length })}
      {waiting > 0 && <b>{t('workspace.orchestratorChat.waiting', { count: waiting })}</b>}
    </button>
  )
}

export function SessionsWaiting({ profileId }: { profileId: string }): React.JSX.Element {
  const { setProject, openPane, project } = useShell()
  const live = useLiveSessions(profileId)
  const [terminal, setTerminal] = useState<LiveSession | null>(null)
  return (
    <>
      <WaitingPrompts profileId={profileId} sessions={live} onAnswered={() => refreshSessions(profileId)} onOpen={setTerminal} />
      <ReplyDrafts profileId={profileId} sessions={live} onDone={() => refreshSessions(profileId)} />
      <StopsAsked profileId={profileId} sessions={live} onDone={() => refreshSessions(profileId)} />
      <ProjectProposals />
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
