import { memo, useCallback, useLayoutEffect, useMemo, useRef, useState } from 'react'

import type { Message } from '../gen/bindings'
import { steeredAnswers } from './activity'
import { continuable, retryable, unanswered } from './chat'
import { useMessage, useMessagesPick, type ChatMessages } from './chatMessages'
import { LimitBanner } from './LimitBanner'
import { Turn } from './Turn'
import { outcome, statusOf, type Outcome } from './turnFold'
import type { Reveal } from './useJump'
import { estimate, GAP, useWindow } from './useWindow'
import { t } from '../i18n'

/* A message's words, as the person sent them. */
const textOf = (message: Message | undefined): string => message?.parts.map((part) => ('text' in part ? part.text : '')).join('') ?? ''

const idsOf = (all: readonly Message[]): string[] => all.map((one) => one.id)

/* The messages a steer never reached the agent with, as the turns that ended say. */
function lostOf(all: readonly Message[]): string[] {
  const lost: string[] = []
  for (const one of all) if (one.ended?.unheard?.length) lost.push(...one.ended.unheard)
  return lost
}

/*
 * The conversation's messages, as a window over them: the rows near the view
 * are drawn, the rest are spacers of their height. A row is wrapped so it can
 * be measured whatever its turn draws.
 *
 * The thread reads only which messages there are; each row reads its own
 * message, so a streamed batch draws the live turn and nothing else.
 */
export function ChatThread({
  messages,
  box,
  top,
  older,
  stalled,
  onMore,
  sending,
  error,
  folder,
  rewindable,
  onRewind,
  onEdit,
  onOpen,
  onContinue,
  onRetry,
  onResend,
  reveal,
}: {
  messages: ChatMessages
  box: React.RefObject<HTMLDivElement | null>
  top: React.RefObject<HTMLDivElement | null>
  older: boolean
  /** A page came back with nothing new: asking again is the person's. */
  stalled: boolean
  onMore: () => void
  sending: boolean
  error: string | null
  folder: string | null
  rewindable: readonly string[]
  onRewind: (turnId: string) => void
  /** Goes back to before a turn, its message to edit. */
  onEdit: (turnId: string) => void
  onOpen: (path: string) => void
  /** Picks the conversation up where its last turn stopped. */
  onContinue: () => void
  /** Sends again the message whose turn failed. */
  onRetry: (text: string) => void
  /** Sends again the words of a message steered in that never reached the agent. */
  onResend?: (text: string) => void
  /** Where the thread hands up how to bring a message into view. */
  reveal?: React.RefObject<Reveal | null>
}): React.JSX.Element {
  const ids = useMessagesPick(messages, idsOf)
  const lost = useMessagesPick(messages, lostOf)
  const steered = useMessagesPick(messages, steeredAnswers)
  const end = useMessagesPick(messages, (all) => ({ unanswered: unanswered(all, sending), continuable: continuable(all, sending), retry: retryable(all, sending), limit: all.at(-1)?.ended?.limit ?? null }))
  const guess = useCallback((id: string) => estimate(messages.byId(id)), [messages])
  const view = useWindow(ids, guess, box)
  /* Sent again from here: not offered twice. */
  const [resent, setResent] = useState<ReadonlySet<string>>(() => new Set())
  const resend = useCallback(
    (id: string) => {
      setResent((was) => new Set(was).add(id))
      onResend?.(textOf(messages.byId(id)))
    },
    [messages, onResend],
  )
  const again = useCallback((id: string) => onResend?.(textOf(messages.byId(id))), [messages, onResend])
  const offered = useMemo(() => new Set(onResend ? lost.filter((id) => !resent.has(id)) : []), [lost, resent, onResend])
  useLayoutEffect(() => {
    if (reveal) reveal.current = view.reveal
  }, [reveal, view.reveal])

  return (
    /* A log, kept quiet: read out, every streamed word and every row the
       window draws would be spoken. The announcer speaks for it. */
    <div className="thread" data-windowed="" role="log" aria-label={t('sessions.chat.conversation')} aria-live="off">
      {older && <div ref={top} className="thread__older" aria-hidden />}
      {older && stalled && <button className="thread__more" onClick={onMore}>{t('sessions.chat.loadEarlier')}</button>}
      {view.before > 0 && <div className="thread__gap" style={{ height: view.before - GAP }} aria-hidden />}
      {ids.slice(view.from, view.to).map((id) => (
        <Row
          key={id}
          id={id}
          messages={messages}
          measure={view.measure}
          folder={folder}
          onOpen={onOpen}
          rewindable={rewindable}
          onRewind={onRewind}
          onEdit={onEdit}
          onAgain={onResend ? again : undefined}
          onResend={offered.has(id) ? resend : undefined}
          steered={steered.includes(id)}
        />
      ))}
      {view.after > 0 && <div className="thread__gap" style={{ height: view.after - GAP }} aria-hidden />}
      {end.unanswered && <p className="said__cmd">{t('sessions.chat.unanswered')}</p>}
      {end.limit && end.retry !== null ? (
        <LimitBanner limit={end.limit} onRetry={() => onRetry(end.retry!)} />
      ) : (end.continuable || end.retry !== null) && (
        <div className="thread__end">
          {end.continuable && <button className="thread__go" onClick={onContinue}>{t('sessions.chat.continue')}</button>}
          {end.retry !== null && <button className="thread__go" onClick={() => onRetry(end.retry!)}>{t('sessions.chat.retry')}</button>}
        </div>
      )}
      {error && <div className="exempty__t" role="alert">{error}</div>}
      <Announcer messages={messages} />
    </div>
  )
}

const working = (): string => t('sessions.chat.working')
const ended = (outcome: Outcome): string => t(`sessions.chat.ended.${outcome}`)

type Last = { id: string; streaming: boolean; said: string | null } | null

function lastAnswer(all: readonly Message[]): Last {
  const last = all.findLast((one) => one.role === 'assistant')
  if (!last) return null
  return { id: last.id, streaming: last.streaming, said: last.ended ? [ended(outcome(last.ended)), statusOf(last)].filter(Boolean).join(' ') : null }
}

/* What changed between two reads of the last answer, in words: one starting, or the one that ran ending. */
function announced(before: Last, last: Last): string | null {
  if (!last || (before?.id === last.id && before.streaming === last.streaming)) return null
  if (last.streaming) return working()
  return before?.id === last.id ? last.said : null
}

/* What a screen reader hears of the turns: that one started and how it ended. */
function Announcer({ messages }: { messages: ChatMessages }): React.JSX.Element {
  const last = useMessagesPick(messages, lastAnswer)
  const [seen, setSeen] = useState(last)
  const [said, setSaid] = useState(() => (last?.streaming ? working() : ''))
  if (seen !== last) {
    setSeen(last)
    const next = announced(seen, last)
    if (next) setSaid(next)
  }
  return (
    <div className="sr-only" role="status" aria-live="polite">
      {said}
    </div>
  )
}

const speaker = (role: Message['role']): string => (role === 'user' ? t('sessions.chat.you') : role === 'assistant' ? t('sessions.chat.agent') : t('sessions.chat.note'))

/* One message of the window, drawn again only when that message is replaced. */
const Row = memo(function Row({
  id,
  messages,
  measure,
  folder,
  onOpen,
  rewindable,
  onRewind,
  onEdit,
  onAgain,
  onResend,
  steered,
}: {
  id: string
  messages: ChatMessages
  measure: (id: string, row: HTMLElement) => void
  folder: string | null
  onOpen: (path: string) => void
  rewindable: readonly string[]
  onRewind: (turnId: string) => void
  onEdit: (turnId: string) => void
  onAgain?: (id: string) => void
  onResend?: (id: string) => void
  steered: boolean
}): React.JSX.Element | null {
  const message = useMessage(messages, id)
  const row = useRef<HTMLDivElement>(null)
  /* A turn that grew on its own: the window learns its height without drawing the thread. */
  useLayoutEffect(() => {
    if (row.current) measure(id, row.current)
  })
  if (!message) return null
  return (
    <div className="trow" data-id={id} ref={row} role="article" aria-label={speaker(message.role)} tabIndex={-1}>
      <Turn
        message={message}
        folder={folder}
        onOpen={onOpen}
        onRewind={message.turnId && rewindable.includes(message.turnId) ? onRewind : undefined}
        onEdit={onEdit}
        onAgain={onAgain ? () => onAgain(id) : undefined}
        onResend={onResend ? () => onResend(id) : undefined}
        steered={steered}
      />
    </div>
  )
})
