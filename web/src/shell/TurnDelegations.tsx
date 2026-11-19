import { useContext, useState } from 'react'

import type { LiveSession } from '../gen/bindings'

import type { Part } from '../gen/bindings'
import { stateWord, stateOf, useLiveSessions } from './liveStatus'
import { AppsScopeContext } from './mcpApps'
import { SessionTerminal } from './SessionTerminal'
import { useShell } from './useShell'
import { t } from '../i18n'

interface Sent {
  id: string
  to: string
  summary: string
  message: string
  /* Why it did not go; null for a send that went or is still going. */
  failed: string | null
}

/** Why a send did not go, from its result. SendMessage answers one that did not
 * with `success: false`, not with an error. */
export function notSent(result: { output: string; is_error: boolean } | undefined): string | null {
  if (!result) return null
  let said: { success?: unknown; message?: unknown } | null = null
  try {
    said = JSON.parse(result.output) as { success?: unknown; message?: unknown }
  } catch {
    said = null
  }
  if (!result.is_error && said?.success !== false) return null
  const why = typeof said?.message === 'string' ? said.message : result.output
  return why.split('\n')[0]?.trim() || 'not sent'
}

/** The sends of a turn, a failed one left out when the same session was sent to again after it. */
export function sendsOf(parts: readonly Part[]): Sent[] {
  const results = new Map<string, { output: string; is_error: boolean }>()
  for (const part of parts) if (part.kind === 'tool_result') results.set(part.call_id, part)
  const all = parts.flatMap((part): Sent[] => {
    if (part.kind !== 'tool_call' || part.parent || part.name !== 'SendMessage') return []
    try {
      const input = JSON.parse(part.input) as { to?: string; summary?: string; message?: string }
      if (!input.to) return []
      const failed = notSent(results.get(part.id)) ?? (part.state === 'failed' ? 'not sent' : null)
      return [{ id: part.id, to: input.to, summary: input.summary ?? '', message: input.message ?? '', failed }]
    } catch {
      return []
    }
  })
  return all.filter((one, at) => !one.failed || !all.slice(at + 1).some((later) => later.to === one.to))
}

/*
 * What a turn handed to other sessions, as cards: to whom, what was asked,
 * and how that session stands now. A message sent is work somebody else is
 * doing, and the row it used to be folded into said none of that.
 */
export function TurnDelegations({ parts }: { parts: readonly Part[] }): React.JSX.Element | null {
  const scope = useContext(AppsScopeContext)
  const sent = sendsOf(parts)
  const sessions = useLiveSessions(sent.some((one) => !one.failed) ? (scope?.profileId ?? null) : null)
  if (sent.length === 0) return null

  return (
    <div className="deleg">
      {sent.map((one) =>
        one.failed ? (
          <Failed key={one.id} to={one.to} summary={one.summary} why={one.failed} />
        ) : (
          <Delegation key={one.id} to={one.to} summary={one.summary} message={one.message} state={stateOf(sessions, one.to)} session={sessions.find((live) => live.name === one.to) ?? null} profileId={scope?.profileId ?? null} />
        ),
      )}
    </div>
  )
}

/* A send that never reached the session: not work anyone is doing. */
function Failed({ to, summary, why }: { to: string; summary: string; why: string }): React.JSX.Element {
  return (
    <div className="deleg__card" data-state="failed">
      <div className="deleg__h">
        <span className="deleg__to">→ {to}</span>
        <span className="deleg__what">{summary || t('sessions.deleg.aMessage')}</span>
        <span className="deleg__state">
          <i className="deleg__dot" />
          {t('sessions.deleg.notSent')}
        </span>
      </div>
      <p className="deleg__msg">{why}</p>
    </div>
  )
}

function Delegation({ to, summary, message, state, session, profileId }: { to: string; summary: string; message: string; state: Parameters<typeof stateWord>[0]; session: LiveSession | null; profileId: string | null }): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const [terminal, setTerminal] = useState(false)
  const { setProject, openPane } = useShell()
  return (
    <div className="deleg__card" data-state={state}>
      <button className="deleg__h" aria-expanded={open} onClick={() => setOpen((was) => !was)}>
        <span className="deleg__to">→ {to}</span>
        <span className="deleg__what">{summary || t('sessions.deleg.aMessage')}</span>
        <span className="deleg__state">
          <i className="deleg__dot" />
          {stateWord(state)}
        </span>
      </button>
      {open && <p className="deleg__msg">{message}</p>}
      {open && session?.pane && (
        <div className="deleg__acts">
          <button className="btn" onClick={() => setTerminal(true)}>{t('sessions.deleg.openTerminal')}</button>
        </div>
      )}
      {terminal && session && (
        <SessionTerminal
          session={session}
          profileId={profileId}
          onClose={() => setTerminal(false)}
          onGo={() => {
            setTerminal(false)
            if (session.projectId) setProject(session.projectId)
            if (session.pane) openPane(session.pane.paneId)
          }}
        />
      )}
    </div>
  )
}
