import { useEffect, useState } from 'react'

import type { Conversations, LivePane, LiveSession, RemoteDraft, RemoteIn, RemoteOut, RemoteTalks } from '../gen/bindings'
import { RemoteTalk } from './RemoteTalk'
import type { Talking } from './talk'
import { t } from '../i18n'

/*
 * Who there is to talk to: the orchestrators, the sessions of their accounts,
 * and this project's chats. Each opens as a chat; a session's terminal is a
 * tap away from it.
 */

const standing = (session: LiveSession): string =>
  session.waiting
    ? t('remote.chats.waiting')
    : session.draft
      ? t('remote.chats.draft')
      : session.status === 'busy'
        ? t('remote.chats.working')
        : t('remote.chats.idle')

export function RemoteChats({
  project,
  chats,
  talks,
  drafts,
  typing,
  send,
  hears,
  onTerminal,
}: {
  project: string | null
  chats: Conversations | null
  talks: RemoteTalks | null
  drafts: readonly RemoteDraft[]
  typing: boolean
  send: (message: RemoteIn) => void
  hears: React.MutableRefObject<(heard: RemoteOut) => void>
  onTerminal: (pane: LivePane) => void
}): React.JSX.Element {
  const [talking, setTalking] = useState<Talking | null>(null)

  useEffect(() => {
    if (talking) return
    send({ t: 'talks' })
    const again = window.setInterval(() => send({ t: 'talks' }), 15_000)
    return () => window.clearInterval(again)
  }, [talking, send])

  if (talking)
    return (
      <RemoteTalk
        talking={talking}
        typing={typing}
        voice={talks?.voice ?? false}
        drafts={drafts}
        send={send}
        hears={hears}
        onBack={() => setTalking(null)}
        onTerminal={onTerminal}
      />
    )

  return (
    <div className="rm__list">
      {!talks && <p className="rm__said">{t('remote.chats.reading')}</p>}
      {talks?.orchestrators.map((one) => (
        <button
          key={one.project}
          className="rm__row"
          onClick={() => setTalking({ kind: 'chat', project: one.project, conversation: one.conversation, title: one.name, orchestrator: one.profile })}
        >
          <span>{one.name}</span>
          <span className="rm__dim">{t('remote.chats.orchestrator')}</span>
        </button>
      ))}
      {talks?.sessions.map(({ profile, session }) => (
        <button
          key={`${profile}/${session.name}/${session.pid}`}
          className="rm__row"
          onClick={() =>
            setTalking({ kind: 'session', profile, name: session.name, title: session.name, project: session.projectId, pane: session.pane })
          }
        >
          <span>{session.name}</span>
          <span className="rm__dim">
            {session.projectName ? `${session.projectName} · ` : ''}
            {standing(session)}
          </span>
        </button>
      ))}
      {project &&
        chats?.conversations.map((one) => (
          <button
            key={one.id}
            className="rm__row"
            onClick={() => setTalking({ kind: 'chat', project, conversation: one.id, title: one.title || t('remote.chats.untitled'), orchestrator: null })}
          >
            <span>{one.title || t('remote.chats.untitled')}</span>
            <span className="rm__dim">{t('remote.chats.chat')}</span>
          </button>
        ))}
      {talks && talks.orchestrators.length === 0 && talks.sessions.length === 0 && !chats?.conversations.length && (
        <p className="rm__said">{t('remote.chats.nobody')}</p>
      )}
    </div>
  )
}
