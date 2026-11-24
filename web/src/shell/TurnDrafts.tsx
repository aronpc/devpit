import { useState } from 'react'

import type { LiveSession, Part } from '../gen/bindings'
import { ask, commands } from './live'
import { refreshSessions, useLiveSessions } from './liveStatus'
import { useShellPick } from './shellStore'
import { WaitingPrompts } from './WaitingPrompts'
import { t } from '../i18n'
import { formatDate } from '../i18n/format'

/*
 * A reply the orchestrator drafted, as a card where it drafted it: the person
 * sends it, edits it or drops it from the chat. Only that click types into the
 * session, as the person — the orchestrator's own words approve nothing.
 */

export interface Drafted {
  readonly id: string
  readonly to: string
  readonly text: string
}

/* The drafts a turn made, from its `devpit_draft_reply` calls. */
export function draftsIn(parts: readonly Part[]): Drafted[] {
  return parts.flatMap((part) => {
    if (part.kind !== 'tool_call' || part.parent || !part.name.endsWith('devpit_draft_reply')) return []
    try {
      const input = JSON.parse(part.input) as { name?: string; text?: string }
      return input.name && input.text ? [{ id: part.id, to: input.name, text: input.text.trim() }] : []
    } catch {
      return []
    }
  })
}

export type Standing = 'pending' | 'sent' | 'dropped' | 'gone'

/* Pending only while it is still the session's draft: a newer one, or one
   sent or dropped elsewhere, leaves this card with nothing to send. */
export function standing(draft: Drafted, session: LiveSession | null, done: Standing | null): Standing {
  if (done) return done
  return session?.draft?.trim() === draft.text ? 'pending' : 'gone'
}

const words = (standing: Standing): string => t(`sessions.drafts.${standing}`)

export function TurnDrafts({ parts }: { parts: readonly Part[] }): React.JSX.Element | null {
  // Drafts are an orchestrator's, for the sessions of its own account.
  const orchestrator = useShellPick((shell) => shell.project?.orchestrator ?? null)
  const drafts = draftsIn(parts)
  const profileId = drafts.length > 0 ? orchestrator : null
  const sessions = useLiveSessions(profileId)
  if (drafts.length === 0 || !profileId) return null
  return (
    <div className="tdraft">
      {drafts.map((one) => (
        <DraftCard key={one.id} draft={one} session={sessions.find((live) => live.name === one.to) ?? null} profileId={profileId} />
      ))}
    </div>
  )
}

function DraftCard({ draft, session, profileId }: { draft: Drafted; session: LiveSession | null; profileId: string }): React.JSX.Element {
  const [done, setDone] = useState<Standing | null>(null)
  const [at, setAt] = useState<string | null>(null)
  const [editing, setEditing] = useState(false)
  const [text, setText] = useState(draft.text)
  const [why, setWhy] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const now = standing(draft, session, done)
  const onAQuestion = Boolean(session?.waiting)

  const send = (): void => {
    setBusy(true)
    void ask(() => commands.orchestratorReply(profileId, draft.to, text, 'the chat')).then((sent) => {
      setBusy(false)
      setWhy(sent.error)
      if (sent.error) return
      setDone('sent')
      setAt(formatDate(new Date(), { hour: '2-digit', minute: '2-digit' }))
      refreshSessions(profileId)
    })
  }
  const drop = (): void => {
    void commands.orchestratorDraftDrop(profileId, draft.to).then(() => {
      setDone('dropped')
      refreshSessions(profileId)
    })
  }

  return (
    <div className="tdraft__card" data-standing={now}>
      <p className="tdraft__h">
        <span className="tdraft__to">{t('sessions.drafts.reply', { name: draft.to })}</span>
        <span className="tdraft__state">{now === 'sent' && at ? t('sessions.drafts.sentAt', { time: at }) : words(now)}</span>
      </p>
      {editing && now === 'pending' ? (
        <textarea className="rdraft__text" aria-label={t('sessions.drafts.reply', { name: draft.to })} value={text} rows={Math.min(6, text.split('\n').length + 1)} onChange={(event) => setText(event.target.value)} />
      ) : (
        <p className="tdraft__text">{text}</p>
      )}
      {now === 'pending' && onAQuestion && session && (
        <WaitingPrompts profileId={profileId} sessions={[session]} onAnswered={() => refreshSessions(profileId)} />
      )}
      {now === 'pending' && (
        <div className="wprompt__opts">
          <button
            className="wprompt__o"
            disabled={busy || onAQuestion || !session?.pane || text.trim() === ''}
            title={onAQuestion ? t('sessions.drafts.onQuestion') : session?.pane ? t('sessions.drafts.types') : t('sessions.drafts.noTerminal')}
            onClick={send}
          >
            {t('sessions.drafts.send')}
          </button>
          <button className="wprompt__o" onClick={() => setEditing((was) => !was)}>{editing ? t('sessions.drafts.doneEditing') : t('sessions.drafts.edit')}</button>
          <button className="wprompt__o wprompt__esc" disabled={busy} onClick={drop}>{t('sessions.drafts.drop')}</button>
        </div>
      )}
      {why && <p className="osess__said">{why}</p>}
    </div>
  )
}
