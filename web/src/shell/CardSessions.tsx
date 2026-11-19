import { useEffect, useState } from 'react'

import type { CardHappening, CardSession } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { adoptable } from './sessionRows'
import { useShell } from './useShell'
import { onCarried } from './window'

/** How long a card's sessions have to be quiet before the card is read again. */
export const READ_AFTER_MS = 250
/** How often a card with a session working reads again, for the step it is on. */
export const STEP_EVERY_MS = 4000

const kindLabel = (kind: CardSession['kind']): string => t(`card.kind.${kind}`)

/** Whether a chat can go on with a session. A run with no agent is heard under
 *  its own id, and no conversation was ever filed under that. */
export const hasConversation = (session: CardSession): boolean => session.ref !== session.runId

/*
 * Every session working on this card, one row each, with what can be done
 * with it from here.
 *
 * Read again whenever the card hears one of them say something, so the rows
 * keep up without the card being closed and opened.
 */
export function CardSessions({
  cardId,
  title,
  sessions,
  onChanged,
}: {
  cardId: string
  title: string
  sessions: readonly CardSession[]
  onChanged: () => void
}): React.JSX.Element | null {
  const { project, show } = useShell()
  const [busy, setBusy] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)

  /* A turn says something many times a second; the card is read once it has
     paused, not once per word. */
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined
    const stop = onCarried<CardHappening>('card:happening', (happening) => {
      if (happening.cardId !== cardId) return
      clearTimeout(timer)
      timer = setTimeout(onChanged, READ_AFTER_MS)
    })
    return () => {
      clearTimeout(timer)
      stop()
    }
  }, [cardId, onChanged])

  /* A state change is what the card hears; a tool after a tool is not one,
     so while something works the step it is on is read again on a beat —
     not for a hidden window, where each read forks tmux and ps for nobody. */
  const working = sessions.some((one) => one.state === 'working')
  useEffect(() => {
    if (!working) return
    const beat = window.setInterval(() => !document.hidden && onChanged(), STEP_EVERY_MS)
    return () => window.clearInterval(beat)
  }, [working, onChanged])

  if (sessions.length === 0) return null
  const projectId = project?.id ?? null

  const doing = async (what: () => Promise<string | null>): Promise<void> => {
    setBusy(true)
    const refused = await what()
    setBusy(false)
    setProblem(refused)
  }

  const goTo = (tabId: string, leafId: string) =>
    doing(async () => {
      if (!projectId) return t('card.noProject')
      show('term', { id: tabId, title, cardId })
      return (await ask(() => commands.sessionFocus(projectId, tabId, leafId))).error
    })

  const asChat = (session: CardSession) =>
    doing(async () => {
      if (!projectId) return t('card.noProject')
      const found = await ask(() => commands.agentProfiles())
      const runnable = (found.data ?? []).filter((profile) => profile.reach === 'runnable' && profile.driver === 'claude')
      const profile = runnable.find((one) => !one.mine) ?? runnable[0]
      if (!profile) return t('card.noProfile')
      const adopted = await ask(() => commands.chatAdopt(projectId, session.ref, profile.id, null, cardId))
      if (!adopted.data) return adopted.error ?? t('card.chatFailed')
      show('chat', { id: adopted.data })
      onChanged()
      return null
    })

  const stop = (runId: string) =>
    doing(async () => {
      const answer = await ask(() => commands.runCancel(cardId, runId))
      onChanged()
      return answer.error
    })

  const attach = () =>
    doing(async () => {
      if (!projectId) return t('card.noProject')
      const answer = await ask(() => commands.terminalAttachAgent(projectId, cardId))
      if (!answer.data) return answer.error ?? t('card.attachFailed')
      show('term', { id: answer.data.tabId, title, cardId: answer.data.cardId })
      return null
    })

  return (
    <section>
      <h2 className="cardp__h">{t('card.sessions')}</h2>
      {sessions.map((session) => {
        const adoption = adoptable(session.kind, session.state)
        const { tabId, leafId, runId } = session
        return (
          <div className="crun" key={`${session.kind}:${session.ref}`} aria-label={kindLabel(session.kind)}>
            <span className="crun__b">
              <span className="crun__t">{kindLabel(session.kind)}</span>
            </span>
            <span className="crun__s" title={session.step ?? undefined}>
              {session.state ?? t('card.notHeard')}
              {session.step && session.state === 'working' && ` · ${session.step}`}
            </span>
            {tabId && leafId && (
              <button className="btn" disabled={busy} onClick={() => void goTo(tabId, leafId)}>
                {t('card.goToTerminal')}
              </button>
            )}
            {session.kind === 'chat' && (
              <button className="btn" disabled={busy} onClick={() => show('chat', { id: session.ref })}>
                {t('card.openChat')}
              </button>
            )}
            {session.kind === 'background' && (
              <button className="btn" disabled={busy} onClick={() => void attach()}>
                {t('card.attach')}
              </button>
            )}
            {session.kind === 'run' && session.state === 'working' && runId && (
              <button className="btn" data-danger disabled={busy} onClick={() => void stop(runId)}>
                {t('card.stop')}
              </button>
            )}
            {(adoption === 'adopt' || adoption === 'stop-first') && hasConversation(session) && (
              <button
                className="btn"
                disabled={busy || adoption === 'stop-first'}
                title={adoption === 'stop-first' ? t('card.stopFirst') : undefined}
                onClick={() => void asChat(session)}
              >
                {t('card.openAsChat')}
              </button>
            )}
          </div>
        )
      })}
      {problem && <p className="wtb__no">{problem}</p>}
    </section>
  )
}
