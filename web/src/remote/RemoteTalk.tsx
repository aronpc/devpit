import { useEffect, useRef, useState } from 'react'

import type { LivePane, Message, RemoteDraft, RemoteIn, RemoteOut } from '../gen/bindings'
import { actionLabel, acts, doing, headline } from '../shell/acts'
import { joined } from '../shell/frames'
import { compactedAt } from '../shell/turnFold'
import { RemoteComposer } from './RemoteComposer'
import { after, asked, draftsOf, type Talking } from './talk'
import { t } from '../i18n'

/*
 * One conversation, or one session, on the phone: what was said, the answer
 * as it streams, the replies drafted for the person to send, and a composer.
 * The rows and the reducer are the window's own; the drawing is a phone's.
 */

/** How often what is shown is read again while nothing streams. A session
    has no frames to stream, so its transcript is read more often. */
const AGAIN_MS = { chat: 8000, session: 4000 } as const

export function RemoteTalk({
  talking,
  typing,
  voice,
  drafts,
  send,
  hears,
  onBack,
  onTerminal,
}: {
  talking: Talking
  typing: boolean
  voice: boolean
  drafts: readonly RemoteDraft[]
  send: (message: RemoteIn) => void
  /** Where what the machine says about this reaches it. */
  hears: React.MutableRefObject<(heard: RemoteOut) => void>
  onBack: () => void
  onTerminal: (pane: LivePane) => void
}): React.JSX.Element {
  const [messages, setMessages] = useState<readonly Message[]>([])
  const [read, setRead] = useState(false)
  const composer = useRef<(heard: RemoteOut) => void>(() => {})
  const thread = useRef<HTMLDivElement>(null)
  const busy = messages.some((one) => one.streaming)
  const streaming = useRef(busy)
  useEffect(() => {
    streaming.current = busy
  }, [busy])

  useEffect(() => {
    const again = (): void => send(asked(talking))
    again()
    send({ t: 'drafts' })
    const timer = window.setInterval(() => !streaming.current && again(), AGAIN_MS[talking.kind])
    hears.current = (heard) => {
      setMessages((was) => after(was, talking, heard))
      if (heard.t === talking.kind) setRead(true)
      // The turn is written down when it ends, with whatever it drafted.
      if (heard.t === 'chatFrame' && heard.frame.type === 'ended') {
        again()
        send({ t: 'drafts' })
      }
      composer.current(heard)
    }
    return () => {
      window.clearInterval(timer)
      hears.current = () => {}
    }
  }, [talking, send, hears])

  useEffect(() => {
    const box = thread.current
    if (box) box.scrollTop = box.scrollHeight
  }, [messages])

  const mine = draftsOf(talking, drafts)

  return (
    <div className="rm__talk">
      <div className="rm__talkh">
        <button className="rm__btn" onClick={onBack}>
          {t('remote.talk.back')}
        </button>
        <span className="rm__host">{talking.title}</span>
        {talking.kind === 'session' && talking.pane && (
          <button className="rm__btn" onClick={() => onTerminal(talking.pane!)}>
            {t('remote.talk.terminal')}
          </button>
        )}
      </div>
      <div className="rm__thread" ref={thread}>
        {!read && <p className="rm__said">{t('remote.talk.reading')}</p>}
        {read && messages.length === 0 && <p className="rm__said">{t('remote.talk.nothing')}</p>}
        {messages.map((message, at) => (
          <RemoteTurn key={talking.kind === 'chat' ? message.id : at} message={message} />
        ))}
      </div>
      {mine.map((draft) => (
        <div className="rm__ask" key={`${draft.profile}/${draft.name}`}>
          <span className="rm__dim">{t('remote.talk.drafted', { name: draft.name })}</span>
          <code className="rm__what">{draft.text}</code>
          {typing && (
            <div className="rm__acts">
              <button className="rm__btn rm__btn--go" onClick={() => confirm(t('remote.talk.confirmDraft', { name: draft.name })) && send({ t: 'draftSend', profile: draft.profile, name: draft.name })}>
                {t('remote.talk.send')}
              </button>
            </div>
          )}
        </div>
      ))}
      <RemoteComposer talking={talking} typing={typing} voice={voice} busy={busy} send={send} hears={composer} />
    </div>
  )
}

/* One message: the person's words, or what the agent did, said and is at. */
function RemoteTurn({ message }: { message: Message }): React.JSX.Element {
  const words = joined(message.parts)
    .flatMap((part) => (part.kind === 'text' && !part.parent ? [part.text] : []))
    .join('\n\n')
  if (message.role === 'user')
    return (
      <div className="rm__msg" data-role="user">
        <p>{words}</p>
      </div>
    )
  const receipt = message.parts.find((part) => part.kind === 'receipt')
  if (receipt?.kind === 'receipt') return <p className="rm__dim">{t(receipt.allowed ? 'remote.talk.allowed' : 'remote.talk.refused', { tool: receipt.tool })}</p>
  const compacted = message.parts.find((part) => part.kind === 'compacted')
  if (compacted?.kind === 'compacted') return <p className="rm__dim">{compactedAt(compacted.tokens)}</p>
  const rows = acts(message.parts)
  const did = headline(rows, message.streaming)
  return (
    <div className="rm__msg" data-role="assistant">
      {did && (
        <details className="rm__did">
          <summary>{did}</summary>
          {rows.map((row) => (
            <div key={row.id} className="rm__dim">
              {actionLabel(row.kind)} {row.target}
            </div>
          ))}
        </details>
      )}
      {words && <p>{words}</p>}
      {message.streaming && <p className="rm__going">{doing(message.parts)}…</p>}
    </div>
  )
}
